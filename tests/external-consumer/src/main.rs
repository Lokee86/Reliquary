use reliquary_memory::{
    Gateway, HostCredentialBinding, MemoryHistoryLookup, MemoryId, OwnerGrant, OwnerKind,
    OwnerScope, Phylactery, Rel, TrustedHostAdmission,
};
use serde_json::Value;
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

fn manifest_paths(
    manifest: &Value,
) -> Result<Vec<(String, String, Vec<(String, String)>)>, String> {
    let scripts = manifest
        .get("conversation_scripts")
        .and_then(Value::as_array)
        .ok_or("missing conversation scripts")?;
    let mut result = Vec::new();
    for script in scripts {
        let conversation = field(script, "conversation_id")?.to_owned();
        let owner = field(script, "owner")?.to_owned();
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
        let mut content = std::collections::BTreeMap::new();
        let mut previous: Option<String> = None;
        for turn in turns {
            let id = field(turn, "id")?.to_owned();
            parent.insert(
                id.clone(),
                child_parent.get(&id).cloned().or_else(|| previous.clone()),
            );
            content.insert(id.clone(), field(turn, "content")?.to_owned());
            previous = Some(id);
        }
        if leaves.is_empty() {
            leaves.push(previous.ok_or("empty authored script")?);
        }
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
            let path = ids
                .into_iter()
                .map(|id| {
                    let body = content
                        .get(&id)
                        .cloned()
                        .ok_or_else(|| format!("missing authored body for {id}"))?;
                    Ok((id, body))
                })
                .collect::<Result<Vec<_>, String>>()?;
            result.push((owner.clone(), conversation.clone(), path));
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
    if lantern_page.total != 8 || cascade_page.total != 4 {
        return Err(
            "authored owner allocation differs from 8 Lantern and 4 Cascade conversations".into(),
        );
    }
    let lantern_turns: u64 = lantern_page
        .items
        .iter()
        .map(|item| item.turn_count as u64)
        .sum();
    let cascade_turns: u64 = cascade_page
        .items
        .iter()
        .map(|item| item.turn_count as u64)
        .sum();
    let lantern_authored_turns = infos[0].rel_turn_count.unwrap_or(0);
    let cascade_authored_turns = infos[1].rel_turn_count.unwrap_or(0);
    if lantern_authored_turns < 32 || cascade_authored_turns < 16 {
        return Err("completed cohort is missing authored base turns".into());
    }
    let mut expected_sources = std::collections::BTreeMap::<String, String>::new();
    for (owner, conversation, expected) in &expected_paths {
        let management = match owner.as_str() {
            "rel-lantern" => &lantern_management,
            "rel-cascade" => &cascade_management,
            _ => return Err(format!("unexpected authored owner alias {owner}")),
        };
        let leaf = expected
            .last()
            .ok_or("manifest branch path is empty")?
            .0
            .as_str();
        management
            .select_conversation(conversation, leaf)
            .map_err(|e| e.to_string())?;
        let actual = management.transcript(0, 128).map_err(|e| e.to_string())?;
        let actual: Vec<_> = actual
            .items
            .into_iter()
            .map(|turn| (turn.node_id, turn.content))
            .collect();
        if actual.as_slice() != expected.as_slice() {
            return Err(format!(
                "fresh reader transcript differs from authored path {conversation}/{leaf}"
            ));
        }
        for (node, body) in expected {
            if let Some(previous) = expected_sources.insert(node.clone(), body.clone()) {
                if previous != *body {
                    return Err(format!("manifest changes the authored body for {node}"));
                }
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
    Ok(format!(
        "{{\"rel_lantern\":{{\"owner_id\":{},\"scope\":\"project\",\"conversations\":{},\"turns\":{},\"memories\":{},\"provenance_source_node\":{},\"history_digest\":{}}},\"rel_cascade\":{{\"owner_id\":{},\"scope\":\"connection\",\"conversations\":{},\"turns\":{},\"memories\":{}}},\"phy_a\":{{\"owner_id\":{},\"scope\":\"unspecified\",\"memory_count\":{}}},\"phy_b\":{{\"owner_id\":{},\"scope\":\"unspecified\",\"memory_count\":{}}},\"authored_turns_checked\":{},\"source_bytes_unchanged\":true}}",
        quoted(&ids[0].to_string()),
        lantern_page.total,
        lantern_authored_turns,
        lantern_memories.total,
        quoted(&provenance.source_node_id),
        quoted(&proof.digest),
        quoted(&ids[1].to_string()),
        cascade_page.total,
        cascade_authored_turns,
        cascade_memories.total,
        quoted(&ids[2].to_string()),
        infos[2].memory_count,
        quoted(&ids[3].to_string()),
        infos[3].memory_count,
        expected_sources.len()
    ))
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
