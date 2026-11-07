use reliquary_memory::{
    ConversationSubmissionId, DurableHostOperationPhase, Gateway, GenerationReplay,
    HostCredentialBinding, MemoryHistoryLookup, MemoryId, OwnerGrant, OwnerKind, OwnerScope,
    Phylactery, QueueState, Rel, RuntimeCapability, SubmissionId, TrustedHostAdmission,
};
use serde_json::{Value, json};
use std::{env, fs, path::Path, process::ExitCode};

fn quoted(value: &str) -> String {
    let mut output = String::from("\"");
    for character in value.chars() {
        match character {
            '"' => output.push_str("\\\""),
            '\\' => output.push_str("\\\\"),
            '\n' => output.push_str("\\n"),
            '\r' => output.push_str("\\r"),
            '\t' => output.push_str("\\t"),
            character if character.is_control() => {
                output.push_str(&format!("\\u{:04x}", character as u32))
            }
            character => output.push(character),
        }
    }
    output.push('"');
    output
}

fn field<'a>(value: &'a Value, name: &str) -> Result<&'a str, String> {
    value
        .get(name)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("missing manifest field {name}"))
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ExpectedTurn {
    node_id: String,
    role: String,
    principal_id: String,
    timestamp_ns: i64,
    content: String,
}

#[derive(Clone, Debug)]
struct ExpectedTranscript {
    owner: String,
    conversation: String,
    leaf: String,
    canonical: bool,
    turns: Vec<ExpectedTurn>,
}

fn manifest_paths(manifest: &Value) -> Result<Vec<ExpectedTranscript>, String> {
    let scripts = manifest
        .get("conversation_scripts")
        .and_then(Value::as_array)
        .ok_or("missing conversation scripts")?;
    let mut result = Vec::new();
    let mut timestamp_ns = 1_i64;
    for script in scripts {
        let conversation = field(script, "conversation_id")?.to_owned();
        let owner = field(script, "owner")?.to_owned();
        let principal = match owner.as_str() {
            "rel-lantern" => "principal-a",
            "rel-cascade" => "principal-b",
            _ => return Err(format!("unexpected authored owner alias {owner}")),
        };
        let turns = script
            .get("turns")
            .and_then(Value::as_array)
            .ok_or("missing script turns")?;
        let branches = script
            .get("branches")
            .and_then(Value::as_array)
            .ok_or("missing script branches")?;
        let mut child_parent = std::collections::BTreeMap::new();
        let mut leaves = Vec::new();
        for branch in branches {
            let parent = field(branch, "parent_turn_id")?.to_owned();
            let children = branch
                .get("child_turn_ids")
                .and_then(Value::as_array)
                .ok_or("missing branch children")?;
            for child in children {
                let child = child.as_str().ok_or("invalid branch child")?.to_owned();
                child_parent.insert(child.clone(), parent.clone());
                leaves.push(child);
            }
        }
        let mut parent = std::collections::BTreeMap::new();
        let mut turns_by_id = std::collections::BTreeMap::new();
        let mut previous: Option<String> = None;
        for turn in turns {
            let id = field(turn, "id")?.to_owned();
            parent.insert(
                id.clone(),
                child_parent.get(&id).cloned().or_else(|| previous.clone()),
            );
            turns_by_id.insert(
                id.clone(),
                ExpectedTurn {
                    node_id: id.clone(),
                    role: field(turn, "speaker")?.to_owned(),
                    principal_id: principal.to_owned(),
                    timestamp_ns,
                    content: field(turn, "content")?.to_owned(),
                },
            );
            timestamp_ns += 1;
            previous = Some(id);
        }
        if leaves.is_empty() {
            leaves.push(previous.ok_or("empty authored script")?);
        }
        let canonical_leaf = branches
            .iter()
            .find_map(|branch| branch.get("canonical_child_id").and_then(Value::as_str));
        for leaf in leaves {
            let mut ids = Vec::new();
            let mut cursor = Some(leaf);
            while let Some(id) = cursor {
                cursor = parent
                    .get(&id)
                    .cloned()
                    .ok_or_else(|| format!("missing parent for {id}"))?;
                ids.push(id);
            }
            ids.reverse();
            let turns = ids
                .into_iter()
                .map(|id| {
                    turns_by_id
                        .get(&id)
                        .cloned()
                        .ok_or_else(|| format!("missing authored turn {id}"))
                })
                .collect::<Result<Vec<_>, String>>()?;
            let leaf = turns
                .last()
                .ok_or("manifest branch path is empty")?
                .node_id
                .clone();
            result.push(ExpectedTranscript {
                owner: owner.clone(),
                conversation: conversation.clone(),
                leaf: leaf.clone(),
                canonical: canonical_leaf.is_none_or(|canonical| canonical == leaf),
                turns,
            });
        }
    }
    Ok(result)
}

fn parse_memory_id(value: &str) -> Result<[u8; 32], String> {
    if value.len() != 64 {
        return Err("Memory ID is not 64 hex characters".into());
    }
    let mut bytes = [0_u8; 32];
    for (index, byte) in bytes.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&value[index * 2..index * 2 + 2], 16)
            .map_err(|error| error.to_string())?;
    }
    Ok(bytes)
}

fn inspect(args: &[String]) -> Result<String, String> {
    if args.len() != 2 {
        return Err("expected completed cohort directory and authored manifest path".into());
    }
    let root = Path::new(&args[0]);
    let paths = [
        root.join("rel-lantern.rel"),
        root.join("rel-cascade.rel"),
        root.join("phy-reader-a.phy"),
        root.join("phy-reader-b.phy"),
    ];
    let source_bytes: Vec<Vec<u8>> = paths
        .iter()
        .map(fs::read)
        .collect::<Result<_, _>>()
        .map_err(|e| format!("cannot read exported cohort: {e}"))?;
    let witness_text = fs::read_to_string(root.join("witness.json"))
        .map_err(|e| format!("cannot read cohort witness: {e}"))?;
    let witness: Value = serde_json::from_str(&witness_text).map_err(|e| e.to_string())?;
    if witness.get("schema_version").and_then(Value::as_u64) != Some(1) {
        return Err("unsupported or missing witness schema version".into());
    }
    let manifest: Value = serde_json::from_slice(&fs::read(&args[1]).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    let expected_paths = manifest_paths(&manifest)?;

    let lantern = Rel::open(&paths[0]).map_err(|error| error.to_string())?;
    let cascade = Rel::open(&paths[1]).map_err(|error| error.to_string())?;
    let phy_a = Phylactery::open(&paths[2]).map_err(|error| error.to_string())?;
    let phy_b = Phylactery::open(&paths[3]).map_err(|error| error.to_string())?;
    let infos = [
        lantern.info().map_err(|e| e.to_string())?,
        cascade.info().map_err(|e| e.to_string())?,
        phy_a.info().map_err(|e| e.to_string())?,
        phy_b.info().map_err(|e| e.to_string())?,
    ];
    let ids = [
        lantern.owner_id(),
        cascade.owner_id(),
        phy_a.owner_id(),
        phy_b.owner_id(),
    ];
    let proofs = [
        lantern.current_historical_proof().map_err(|e| e.to_string())?,
        cascade.current_historical_proof().map_err(|e| e.to_string())?,
        phy_a.current_historical_proof().map_err(|e| e.to_string())?,
        phy_b.current_historical_proof().map_err(|e| e.to_string())?,
    ];
    for (index, info) in infos.iter().enumerate() {
        let kind = if index < 2 {
            OwnerKind::Rel
        } else {
            OwnerKind::Phylactery
        };
        let alias = paths[index].file_name().unwrap().to_string_lossy();
        let entry = witness
            .get("owners")
            .and_then(Value::as_array)
            .and_then(|owners| {
                owners
                    .iter()
                    .find(|owner| owner.get("file").and_then(Value::as_str) == Some(alias.as_ref()))
            });
        if info.kind != kind || entry.is_none() {
            return Err(format!(
                "owner {} identity or kind disagrees with exported witness",
                index + 1
            ));
        }
        let entry = entry.unwrap();
        if entry["memory_count"].as_u64() != Some(info.memory_count as u64)
            || entry["history_digest"].as_str() != Some(proofs[index].digest.as_str())
            || (index < 2 && entry["accepted_sources"].as_u64() != info.rel_turn_count)
        {
            return Err(format!("owner {alias} count or retained history differs after process reopen"));
        }
        let expected_kind = if index < 2 { "rel" } else { "phy" };
        if entry.get("owner_id").and_then(Value::as_str) != Some(ids[index].to_string().as_str())
            || entry.get("kind").and_then(Value::as_str) != Some(expected_kind)
        {
            return Err(format!(
                "owner {} kind or alias differs from exported witness",
                index + 1
            ));
        }
        let expected_scope = match index {
            0 => "project",
            1 => "connection",
            _ => "unspecified",
        };
        if entry.get("scope").and_then(Value::as_str) != Some(expected_scope) {
            return Err(format!(
                "owner {} scope differs from exported witness",
                index + 1
            ));
        }
    }
    if infos[0].scope != OwnerScope::Project || infos[1].scope != OwnerScope::Connection {
        return Err("REL owner scopes differ from the authored Lantern/Cascade assignments".into());
    }
    if infos[2].memory_count == 0 || infos[3].memory_count == 0 {
        return Err("the authored private PHY owners must both contain private memories".into());
    }

    let gateway = Gateway::new();
    let lantern_id = gateway
        .mount_rel(&lantern, true)
        .map_err(|e| e.to_string())?;
    let cascade_id = gateway
        .mount_rel(&cascade, true)
        .map_err(|e| e.to_string())?;
    let phy_a_id = gateway
        .mount_phylactery(&phy_a, false)
        .map_err(|e| e.to_string())?;
    let phy_b_id = gateway
        .mount_phylactery(&phy_b, false)
        .map_err(|e| e.to_string())?;
    let operator_token = "external-g4-operator-token-000000000000000000000";
    let viewer_token = "external-g4-viewer-token-000000000000000000000000";
    let outsider_token = "external-g4-outsider-token-00000000000000000000000";
    let queue_manager_token = "external-g4-queue-manager-token-0000000000000000000000";
    let (adapter, control) = TrustedHostAdmission::new([
        HostCredentialBinding::new(
            "operator",
            operator_token,
            [
                OwnerGrant::ManageRel(lantern_id),
                OwnerGrant::ManageRel(cascade_id),
                OwnerGrant::ViewPhylactery(phy_a_id),
            ],
        ),
        HostCredentialBinding::new(
            "viewer",
            viewer_token,
            [
                OwnerGrant::ViewRel(lantern_id),
                OwnerGrant::ViewRel(cascade_id),
                OwnerGrant::ViewPhylactery(phy_a_id),
            ],
        ),
        HostCredentialBinding::new(
            "outsider",
            outsider_token,
            [OwnerGrant::ViewRel(cascade_id)],
        ),
        HostCredentialBinding::new(
            "principal-a",
            queue_manager_token,
            [
                OwnerGrant::ManageRel(lantern_id),
                OwnerGrant::OperateRel(lantern_id),
            ],
        ),
    ])
    .install(&gateway)
    .map_err(|e| e.to_string())?;
    let operator = adapter
        .authenticate(operator_token)
        .map_err(|e| e.to_string())?;
    let viewer = adapter
        .authenticate(viewer_token)
        .map_err(|e| e.to_string())?;
    let outsider = adapter
        .authenticate(outsider_token)
        .map_err(|e| e.to_string())?;
    let queue_manager = adapter
        .authenticate(queue_manager_token)
        .map_err(|e| e.to_string())?;
    let lantern_management = operator
        .workspace_management(lantern_id)
        .map_err(|e| e.to_string())?;
    let cascade_management = operator
        .workspace_management(cascade_id)
        .map_err(|e| e.to_string())?;
    let lantern_page = lantern_management
        .conversations(0, 128)
        .map_err(|e| e.to_string())?;
    let cascade_page = cascade_management
        .conversations(0, 128)
        .map_err(|e| e.to_string())?;
    if lantern_page.total < 8 || cascade_page.total < 4 {
        return Err(
            "completed cohort is missing the authored 8 Lantern and 4 Cascade conversations".into(),
        );
    }
    let imported_title = cascade_page
        .items
        .iter()
        .find(|conversation| conversation.conversation_id == "conv-c03-additive-import")
        .and_then(|conversation| conversation.title.as_deref());
    if imported_title != Some("Cascade native import") {
        return Err("Cascade native import title differs from the authored manifest result".into());
    }
    let lantern_title = lantern_page
        .items
        .iter()
        .find(|conversation| conversation.conversation_id == "conv-l01")
        .and_then(|conversation| conversation.title.as_deref());
    if lantern_title != Some("Lantern release") {
        return Err("Lantern public management title differs from the accepted title".into());
    }
    let lantern_health = lantern_management.health().map_err(|e| e.to_string())?;
    let cascade_health = cascade_management.health().map_err(|e| e.to_string())?;
    if !lantern_health.admitted
        || !cascade_health.admitted
        || lantern_health.conversations != lantern_page.total
        || cascade_health.conversations != cascade_page.total
    {
        return Err(
            "public REL health does not match the independently reopened conversation pages".into(),
        );
    }
    let lantern_authored_turns = infos[0].rel_turn_count.unwrap_or(0);
    let cascade_authored_turns = infos[1].rel_turn_count.unwrap_or(0);
    if lantern_authored_turns < 32 || cascade_authored_turns < 16 {
        return Err("completed cohort is missing authored base turns".into());
    }
    let mut expected_sources = std::collections::BTreeMap::<String, String>::new();
    for expected in &expected_paths {
        let management = match expected.owner.as_str() {
            "rel-lantern" => &lantern_management,
            "rel-cascade" => &cascade_management,
            _ => {
                return Err(format!(
                    "unexpected authored owner alias {}",
                    expected.owner
                ));
            }
        };
        management
            .select_conversation(&expected.conversation, &expected.leaf)
            .map_err(|e| e.to_string())?;
        let actual = management.transcript(0, 128).map_err(|e| e.to_string())?;
        let actual: Vec<_> = actual
            .items
            .into_iter()
            .map(|turn| ExpectedTurn {
                node_id: turn.node_id,
                role: turn.role,
                principal_id: turn.principal_id.unwrap_or_default(),
                timestamp_ns: turn.timestamp_ns,
                content: turn.content,
            })
            .collect();
        if actual.as_slice() != expected.turns.as_slice() {
            return Err(format!(
                "fresh reader transcript differs from authored path {}/{}",
                expected.conversation, expected.leaf
            ));
        }
        for turn in &expected.turns {
            if let Some(previous) =
                expected_sources.insert(turn.node_id.clone(), turn.content.clone())
            {
                if previous != turn.content {
                    return Err(format!(
                        "manifest changes the authored body for {}",
                        turn.node_id
                    ));
                }
            }
        }
    }
    for page in [&lantern_page, &cascade_page] {
        for conversation in &page.items {
            let extension_leaf = match conversation.conversation_id.as_str() {
                "conv-l06" => Some("source-doc"),
                "conv-l07" => Some("accepted"),
                "conv-c03-additive-import" => Some("C03-t4"),
                "g4-conversation" => Some("accepted-turn"),
                "g4-race" => Some("exclusive-turn"),
                _ => None,
            };
            if let Some(leaf) = extension_leaf {
                if conversation.branch.leaf_node_id != leaf || !conversation.branch.canonical {
                    return Err(format!("completed branch extension differs for {}", conversation.conversation_id));
                }
                continue;
            }
            let selected = expected_paths
                .iter()
                .find(|path| {
                    path.conversation == conversation.conversation_id
                        && path.leaf == conversation.branch.leaf_node_id
                })
                .ok_or_else(|| {
                    format!(
                        "public branch head is not an authored path for {}",
                        conversation.conversation_id,
                    )
                })?;
            if conversation.branch.canonical != selected.canonical {
                return Err(format!(
                    "public branch canonical metadata differs from the manifest for {}",
                    conversation.conversation_id
                ));
            }
        }
    }
    let authored_turns: usize = manifest
        .get("conversation_scripts")
        .and_then(Value::as_array)
        .ok_or("manifest has no scripts")?
        .iter()
        .map(|script| {
            script
                .get("turns")
                .and_then(Value::as_array)
                .map_or(0, Vec::len)
        })
        .sum();
    if expected_sources.len() != authored_turns {
        return Err(format!(
            "fresh reader covered {} of {authored_turns} authored turn nodes",
            expected_sources.len()
        ));
    }

    let lantern_memories = lantern_management
        .memories(0, 128)
        .map_err(|e| e.to_string())?;
    let cascade_memories = cascade_management
        .memories(0, 128)
        .map_err(|e| e.to_string())?;
    if lantern_health.memories != lantern_memories.total
        || cascade_health.memories != cascade_memories.total
    {
        return Err(
            "public REL health Memory totals differ from the bounded management pages".into(),
        );
    }
    let memory = lantern_memories
        .items
        .first()
        .ok_or("Lantern Memory missing")?;
    let memory_id = MemoryId(parse_memory_id(&memory.id)?);
    let direct = lantern_management
        .memory(memory_id)
        .map_err(|e| e.to_string())?;
    if direct.content != memory.content || direct.title != memory.title {
        return Err("direct Memory differs from bounded list result".into());
    }
    let provenance = lantern_management
        .provenance(memory_id)
        .map_err(|e| e.to_string())?;
    if provenance.owner_id != lantern_id.to_string() || provenance.source_node_id.is_empty() {
        return Err("Memory provenance is incomplete".into());
    }
    let history = lantern
        .memory_history(memory_id)
        .map_err(|e| e.to_string())?;
    if !matches!(history, MemoryHistoryLookup::Found(_)) {
        return Err("Memory history missing after independent open".into());
    }
    let proof = lantern
        .current_historical_proof()
        .map_err(|e| e.to_string())?;
    if viewer
        .workspace_management(lantern_id)
        .map_err(|e| e.to_string())?
        .create_memory(
            "denied".into(),
            "denied".into(),
            "fact".into(),
            "note".into(),
            1,
        )
        .is_ok()
    {
        return Err("viewer unexpectedly wrote a Memory".into());
    }
    if outsider.workspace_management(lantern_id).is_ok()
        || viewer.phylactery_info(phy_b_id).is_ok()
        || operator
            .phylactery_info(phy_a_id)
            .map_err(|e| e.to_string())?
            .owner_id
            != ids[2]
    {
        return Err("owner or PHY authorization boundary was not preserved".into());
    }
    control.revoke(viewer_token).map_err(|e| e.to_string())?;
    if viewer
        .workspace_management(lantern_id)
        .and_then(|m| m.conversations(0, 1).map(|_| ()))
        .is_ok()
    {
        return Err("revoked viewer retained read access".into());
    }

    let lifecycle = witness
        .get("lifecycle")
        .ok_or("exported witness has no Gateway lifecycle results")?;
    let race_finalize_won = lifecycle
        .get("race_finalize_won")
        .and_then(Value::as_bool)
        .ok_or("Gateway lifecycle witness has no race winner")?;
    let queue = queue_manager
        .open_conversation_queue(root.join("lifecycle-queue"), lantern_id)
        .map_err(|e| e.to_string())?;
    let submission = |name: &str| -> Result<ConversationSubmissionId, String> {
        ConversationSubmissionId::new(
            "g4-conversation",
            SubmissionId::new(name).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())
    };
    for (name, expected) in [
        ("accepted-turn", QueueState::Committed),
        ("cancelled-turn", QueueState::Cancelled),
    ] {
        let id = submission(name)?;
        let status = queue_manager
            .conversation_submission_status(&queue, &id)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| format!("exported queue is missing {name}"))?;
        if status.state != expected {
            return Err(format!(
                "queue submission {name} has the wrong reopened state"
            ));
        }
    }
    let race_queue = queue_manager
        .open_conversation_queue(root.join("lifecycle-race-queue"), lantern_id)
        .map_err(|e| e.to_string())?;
    let race_id = ConversationSubmissionId::new(
        "g4-race",
        SubmissionId::new("exclusive-turn").map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    let race_status = queue_manager
        .conversation_submission_status(&race_queue, &race_id)
        .map_err(|e| e.to_string())?
        .ok_or("exported race queue is missing its winning submission")?;
    if race_status.state
        != if race_finalize_won {
            QueueState::Committed
        } else {
            QueueState::Cancelled
        }
    {
        return Err("fresh process race state differs from the recorded winner".into());
    }
    let held_id = submission("host-held-turn")?;
    let operation = queue_manager
        .host_operation_status(&queue, &held_id, RuntimeCapability::ConversationExecution)
        .map_err(|e| e.to_string())?
        .ok_or("exported queue is missing the held host operation")?;
    if operation.phase != DurableHostOperationPhase::Held {
        return Err("fresh process did not recover the host operation as Held".into());
    }
    let checkpoint = queue_manager
        .generation_checkpoint_snapshot(&queue, &held_id)
        .map_err(|e| e.to_string())?
        .ok_or("exported queue is missing its generation checkpoint")?;
    if checkpoint.content_prefix != "private provisional prefix" {
        return Err("fresh process checkpoint prefix differs from the durable witness".into());
    }
    let replay = queue_manager
        .replay_generation_checkpoints(&queue, &held_id, 0, 8)
        .map_err(|e| e.to_string())?;
    let GenerationReplay::Checkpoints {
        latest_cursor,
        items,
    } = replay
    else {
        return Err("fresh queue reader requires a resync instead of replay".into());
    };
    if items.len() != 1
        || items[0].content_prefix != checkpoint.content_prefix
        || latest_cursor < items[0].delivery_cursor
    {
        return Err("fresh queue checkpoint replay or resume cursor is inconsistent".into());
    }
    let resumed = queue_manager
        .replay_generation_checkpoints(&queue, &held_id, latest_cursor, 8)
        .map_err(|e| e.to_string())?;
    if !matches!(resumed, GenerationReplay::Checkpoints { items, .. } if items.is_empty()) {
        return Err("checkpoint resume cursor replayed already-consumed private output".into());
    }
    drop((queue, race_queue));

    let queue_manager_management = queue_manager
        .workspace_management(lantern_id)
        .map_err(|e| e.to_string())?;
    queue_manager_management
        .quiesce()
        .map_err(|e| e.to_string())?;
    control
        .revoke(queue_manager_token)
        .map_err(|e| e.to_string())?;
    if queue_manager_management.conversations(0, 1).is_ok()
        || queue_manager_management.reopen().is_ok()
    {
        return Err("revoked manage-capable principal retained REL read or reopen access".into());
    }
    lantern_management.reopen().map_err(|e| e.to_string())?;

    lantern_management.quiesce().map_err(|e| e.to_string())?;
    cascade_management.quiesce().map_err(|e| e.to_string())?;
    if lantern_management
        .verify_quiesced()
        .map_err(|e| e.to_string())?
        .owner_id
        != ids[0]
        || cascade_management
            .verify_quiesced()
            .map_err(|e| e.to_string())?
            .owner_id
            != ids[1]
    {
        return Err("quiesced owner identity changed".into());
    }
    lantern_management.reopen().map_err(|e| e.to_string())?;
    cascade_management.reopen().map_err(|e| e.to_string())?;

    let after: Vec<Vec<u8>> = paths
        .iter()
        .map(fs::read)
        .collect::<Result<_, _>>()
        .map_err(|e| e.to_string())?;
    if after != source_bytes {
        return Err(
            "a public read, denial, revocation, quiesce, or reopen changed source bytes".into(),
        );
    }
    Ok(json!({
        "rel_lantern": {
            "owner_id": ids[0].to_string(), "scope": "project",
            "conversations": lantern_page.total, "turns": lantern_authored_turns,
            "memories": lantern_memories.total,
            "provenance_source_node": provenance.source_node_id,
            "history_digest": proof.digest,
            "health_admitted": lantern_health.admitted,
            "health_conversations": lantern_health.conversations,
            "health_memories": lantern_health.memories,
            "title": lantern_title,
        },
        "rel_cascade": {
            "owner_id": ids[1].to_string(), "scope": "connection",
            "conversations": cascade_page.total, "turns": cascade_authored_turns,
            "memories": cascade_memories.total,
            "health_admitted": cascade_health.admitted,
            "health_conversations": cascade_health.conversations,
            "health_memories": cascade_health.memories,
            "title": imported_title,
        },
        "phy_a": {"owner_id": ids[2].to_string(), "scope": "unspecified", "memory_count": infos[2].memory_count},
        "phy_b": {"owner_id": ids[3].to_string(), "scope": "unspecified", "memory_count": infos[3].memory_count},
        "gateway_queue": {
            "accepted": "committed", "cancelled": "cancelled",
            "race": if race_finalize_won { "committed" } else { "cancelled" },
            "host_operation": "held", "checkpoint_prefix": checkpoint.content_prefix,
            "checkpoint_resume_cursor": latest_cursor,
        },
        "authored_turns_checked": expected_sources.len(),
        "source_bytes_unchanged": true,
    }).to_string())
}

fn main() -> ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();
    match inspect(&args) {
        Ok(report) => {
            println!("{report}");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("{{\"error\":{}}}", quoted(&error));
            ExitCode::FAILURE
        }
    }
}
