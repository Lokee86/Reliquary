//! Opt-in comparison of unchanged authoritative fixture histories.
#[path = "../../crates/reliquary-entity-resolution/src/identity_trial/pilot/signed_in.rs"]
mod signed_in;

use super::{Directory, UnusedEmbedding};
use reliquary_container::Container;
use reliquary_memory::{
    ConfiguredRuntime, EmbeddingNormalization, Gateway, GeneralEndpoint, GeneralEndpointError,
    HostCredentialBinding, HostedEmbeddingInputSemantics, HostedInferenceIdentity,
    IdentityCitation, IdentityComparisonState, IdentityEffectiveDecision, IdentityOutcome,
    IdentityReason, IdentityReceipt, IdentitySide, MemoryId, MemoryTextField, OwnerGrant, OwnerId,
    OwnerKind, TrustedHostAdmission, WorkspaceManagement,
};
use reliquary_owner_session::RelSession;
use reliquary_perception_store::{
    PerceptionStore, RetainedResolutionDecision, RetainedResolutionValue,
    identity_evidence::IdentityEvidence,
};
use reliquary_switchboard::ConfiguredRoutes;
use serde_json::Value;
use signed_in::SignedInSol;
use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

const FIXTURES: [&str; 3] = [
    "chatgpt-first14d",
    "chatgpt-first28d",
    "rhelm-david-r-ellis-pre-relationship",
];

#[test]
#[ignore = "uses RELIQUARY_MENTION_CORPUS_DIR and sends excerpts via authorized signed-in Codex"]
fn authoritative_mention_corpus_compares_and_reopens() {
    let root = PathBuf::from(std::env::var_os("RELIQUARY_MENTION_CORPUS_DIR").unwrap());
    let dir = Directory::new();
    let originals = FIXTURES.map(|name| root.join(name).join("project.prj.rel"));
    let hashes = originals.each_ref().map(|path| checksum(path));
    let copies: [PathBuf; 3] = std::array::from_fn(|index| {
        let copy = dir.0.join(format!("{}.rel", FIXTURES[index]));
        fs::copy(&originals[index], &copy).unwrap();
        assert_eq!(checksum(&copy), hashes[index]);
        copy
    });
    let owners = copies
        .each_ref()
        .map(|path| RelSession::open_path_current(path).unwrap());
    let stores = copies
        .each_ref()
        .map(|path| PerceptionStore::open(&Container::open_read_only(path).unwrap()).unwrap());
    for (index, name) in FIXTURES.iter().enumerate() {
        let session = &owners[index];
        let view = session.management_read();
        assert_eq!(view.relationships().count(), 0);
        assert!(session.entity_resolutions().is_empty());
        let retained = &stores[index];
        let keys = retained
            .retained_resolution_history()
            .iter()
            .filter_map(|row| match &row.value {
                RetainedResolutionValue::Revision(value) => Some(value.key),
                RetainedResolutionValue::Inert(_) => None,
            })
            .collect::<BTreeSet<_>>();
        let resolved = keys
            .into_iter()
            .filter_map(|key| retained.retained_resolution_at_head(key))
            .filter(|head| matches!(head.decision, RetainedResolutionDecision::Resolved { .. }))
            .count();
        assert_eq!(resolved == 0, index == 0);
        println!(
            "{name}: {} entities, {} Memories, zero Relationships, zero ordinary mention decisions, {resolved} current resolved retained decisions",
            view.entities().count(),
            view.memory_count()
        );
    }
    assert_eq!(
        owners[0].management_read().owner_id,
        owners[1].management_read().owner_id,
        "ChatGPT snapshots should represent the same owner"
    );
    assert_ne!(
        owners[1].management_read().owner_id,
        owners[2].management_read().owner_id
    );

    // Pin the sampled Entities by nonidentifying digests of the fixture-local IDs.
    let empty = fixture_evidence(&owners[0], "5599a35ebd58");
    let left = fixture_evidence(&owners[1], "94e7f9833e87");
    let right = fixture_evidence(&owners[2], "397fc31f0a13");
    assert!(empty.mentions.is_empty());
    assert!(independent(&owners[1], &owners[2], &left, &right));
    let fields = [&left, &right].map(|side| {
        side.mentions
            .iter()
            .fold([0, 0], |[title, content], m| match m.key.field {
                MemoryTextField::Title => [title + 1, content],
                MemoryTextField::Content => [title, content + 1],
            })
    });
    let left_id = left.endpoint.id;
    let right_id = right.endpoint.id;
    let empty_id = empty.endpoint.id;
    let quotes = [
        exact_quotes(&owners[1], &stores[1], &left),
        exact_quotes(&owners[2], &stores[2], &right),
    ];
    drop(stores);
    drop(owners);

    let config = std::env::var_os("RELIQUARY_IDENTITY_TRIAL_CONFIG")
        .expect("protected Entity route profile");
    let routes =
        ConfiguredRoutes::from_path(Path::new(&config)).expect("open Entity route profile");
    let selected = routes
        .entity_endpoint()
        .expect("configured Entity general route");
    assert_eq!(selected.model(), "gpt-5.6-sol");
    let endpoint = Arc::new(CorpusEndpoint {
        quotes,
        transport: SignedInSol {
            executable: std::env::var_os("RELIQUARY_TRIAL_CODEX")
                .expect("signed-in Codex executable")
                .into(),
            scratch: dir.0.join("model"),
            model: selected.model().to_owned(),
            retain_client_log: false,
        },
        observation: Mutex::new(None),
    });
    fs::create_dir(&endpoint.transport.scratch).unwrap();
    println!("Protected route profile. {}", routes.readiness());
    let runtime = ConfiguredRuntime::hosted(
        endpoint.clone(),
        Arc::new(UnusedEmbedding),
        HostedInferenceIdentity::new(
            "signed-in-codex",
            endpoint.model(),
            "unused",
            "unused",
            2,
            HostedEmbeddingInputSemantics::Shared,
            EmbeddingNormalization::L2,
        )
        .unwrap(),
    )
    .unwrap();
    println!("Endpoint {}. {}", endpoint.model(), runtime.readiness());
    // The 14-day and 28-day files are snapshots of one owner. Mount separately.
    let empty = {
        let (_gateway, work, peer) = mount(&copies[0], &copies[2], &runtime);
        work.compare_entity_identity(empty_id, peer, right_id)
            .unwrap()
    };
    assert_eq!(empty.outcome, IdentityOutcome::Unresolved);
    assert_eq!(empty.reason, IdentityReason::InsufficientEvidence);
    assert!(empty.left.mentions.is_empty() && empty.citations.is_empty());
    assert!(endpoint.observation.lock().unwrap().is_none());
    {
        let (_gateway, reopened, peer) = mount(&copies[0], &copies[2], &runtime);
        check_receipt(&reopened, peer, &empty, &[vec![], vec![]]);
    }

    let comparison = {
        let (_gateway, work, peer) = mount(&copies[1], &copies[2], &runtime);
        let receipt = work
            .compare_entity_identity(left_id, peer, right_id)
            .unwrap();
        check_receipt(&work, peer, &receipt, &endpoint.quotes);
        receipt
    };
    let input_counts = endpoint.quotes.each_ref().map(Vec::len);
    assert_eq!(comparison.left.mentions.len(), input_counts[0]);
    assert_eq!(comparison.right.mentions.len(), input_counts[1]);
    assert!(comparison.left.relationships.is_empty() && comparison.right.relationships.is_empty());
    let (outcome, reason, citation_count) = endpoint.observation.lock().unwrap().clone().unwrap();
    assert_eq!(comparison.citations.len(), citation_count);
    {
        let (_gateway, reopened, peer) = mount(&copies[1], &copies[2], &runtime);
        check_receipt(&reopened, peer, &comparison, &endpoint.quotes);
        assert_eq!(
            reopened
                .compare_entity_identity(left_id, peer, right_id)
                .unwrap(),
            comparison
        );
    }
    println!(
        "14-day absence control: zero model calls, {:?}/{:?}, fresh reopen passed",
        empty.outcome, empty.reason
    );
    println!(
        "28-day/Ellis: one call, {input_counts:?} exact quotes (title/content counts {fields:?}), raw {outcome}/{reason}, effective {:?}/{:?}, {citation_count} validated citations, fresh reopen passed. Reference label unavailable.",
        comparison.outcome, comparison.reason
    );
    assert_eq!(originals.each_ref().map(|path| checksum(path)), hashes);
    assert_eq!(
        checksum(&copies[2]),
        hashes[2],
        "read-only peer copy changed"
    );
    println!("All three source hashes and the read-only peer copy hash are unchanged.");
}

fn checksum(path: &Path) -> blake3::Hash {
    blake3::Hasher::new()
        .update_reader(fs::File::open(path).unwrap())
        .unwrap()
        .finalize()
}

fn fixture_evidence(owner: &RelSession, digest_prefix: &str) -> IdentityEvidence {
    let view = owner.management_read();
    let mut matching = view.entities().filter(|entity| {
        blake3::hash(&entity.id.0)
            .to_hex()
            .as_str()
            .starts_with(digest_prefix)
    });
    let id = matching.next().expect("fixture Entity is missing").id;
    assert!(
        matching.next().is_none(),
        "fixture Entity digest is ambiguous"
    );
    owner.identity_evidence(id).unwrap().unwrap()
}

fn independent(
    left: &RelSession,
    right: &RelSession,
    a: &IdentityEvidence,
    b: &IdentityEvidence,
) -> bool {
    !a.mentions.is_empty()
        && !b.mentions.is_empty()
        && a.mentions.iter().all(|l| {
            let ls = left
                .current_memory(MemoryId(l.key.memory_id))
                .unwrap()
                .record
                .source_ref
                .unwrap();
            b.mentions.iter().all(|r| {
                let rs = right
                    .current_memory(MemoryId(r.key.memory_id))
                    .unwrap()
                    .record
                    .source_ref
                    .unwrap();
                l.quote != r.quote
                    && l.memory.reference.body_id != r.memory.reference.body_id
                    && ls.source_node_id != rs.source_node_id
                    && ls.source_episode_id != rs.source_episode_id
                    && !l.memory.lineage.is_empty()
                    && !r.memory.lineage.is_empty()
                    && l.memory
                        .lineage
                        .iter()
                        .all(|key| !r.memory.lineage.contains(key))
            })
        })
}

fn exact_quotes(
    owner: &RelSession,
    retained: &PerceptionStore,
    side: &IdentityEvidence,
) -> Vec<String> {
    side.mentions
        .iter()
        .map(|witness| {
            let memory = witness.memory.reference;
            let span = memory.span.unwrap();
            let read = owner.management_read();
            let (record, body) = read
                .memory_projection(MemoryId(memory.id), Some(memory.revision))
                .unwrap();
            let body = body.unwrap();
            let ordinal = witness
                .retained_ordinal
                .expect("quote must come from native retained decision");
            let row = retained
                .retained_resolution_history()
                .iter()
                .find(|row| row.source_ordinal == ordinal)
                .unwrap();
            let RetainedResolutionValue::Revision(mention) = &row.value else {
                panic!("inert resolution must not witness identity");
            };
            assert_eq!(
                retained.retained_resolution_at_head(witness.key),
                Some(mention)
            );
            assert!(
                matches!(mention.decision, RetainedResolutionDecision::Resolved { entity_id, .. }
                if entity_id == side.endpoint.id)
            );
            assert_eq!(mention.revision, witness.revision);
            assert_eq!(record.body_id.0, memory.body_id);
            assert_eq!(record.authority_kind, "direct");
            let history = owner.memory_history(MemoryId(memory.id)).unwrap().unwrap();
            assert!(
                history
                    .revisions
                    .iter()
                    .all(|revision| revision.record.body_id == record.body_id),
                "retained mention source changed across Memory revisions"
            );
            assert!(
                span.field == witness.key.field
                    && span.start_byte <= witness.key.start_byte
                    && witness.key.end_byte <= span.end_byte
            );
            let field = match span.field {
                MemoryTextField::Title => &body.title,
                MemoryTextField::Content => &body.content,
            };
            let exact = field
                .get(span.start_byte as usize..span.end_byte as usize)
                .unwrap();
            assert!(
                exact == witness.quote,
                "captured quote differs from the retained source"
            );
            exact.to_owned()
        })
        .collect()
}

fn mount(
    local: &Path,
    remote: &Path,
    runtime: &ConfiguredRuntime,
) -> (Gateway, WorkspaceManagement, OwnerId) {
    let gateway = Gateway::new();
    let owner = gateway
        .mount_owner_path(local, OwnerKind::Rel, true, None)
        .unwrap();
    let peer = gateway
        .mount_owner_path(remote, OwnerKind::Rel, false, None)
        .unwrap();
    gateway.set_runtime(Some(runtime)).unwrap();
    let token = "local-mention-corpus-test-credential";
    let (admission, _) = TrustedHostAdmission::new([HostCredentialBinding::new(
        "local-mention-corpus-test",
        token,
        [OwnerGrant::ManageRel(owner), OwnerGrant::ViewRel(peer)],
    )])
    .install(&gateway)
    .unwrap();
    let work = admission
        .authenticate(token)
        .unwrap()
        .workspace_management(owner)
        .unwrap();
    (gateway, work, peer)
}

fn check_receipt(
    work: &WorkspaceManagement,
    owner: OwnerId,
    expected: &IdentityReceipt,
    quotes: &[Vec<String>; 2],
) {
    let view = work
        .inspect_entity_identity(
            expected.pair.local_entity,
            owner,
            expected.pair.peer_entity,
            None,
        )
        .unwrap()
        .unwrap();
    assert_eq!(view.receipt, *expected);
    assert_eq!(view.state, IdentityComparisonState::Current);
    assert_eq!(
        view.effective,
        IdentityEffectiveDecision::Evaluated(expected.outcome)
    );
    let cited = view.cited_quotes.unwrap();
    assert_eq!(cited.len(), expected.citations.len());
    for (quote, citation) in cited.iter().zip(&expected.citations) {
        let IdentityCitation::Mention { side, mention } = citation else {
            panic!("receipt with zero Relationships contains a Relationship citation");
        };
        let side = usize::from(*side == IdentitySide::Right);
        assert!(
            quote == &quotes[side][usize::from(*mention)],
            "reopened quote differs from the supplied evidence"
        );
    }
}

struct CorpusEndpoint {
    quotes: [Vec<String>; 2],
    transport: SignedInSol,
    observation: Mutex<Option<(String, String, usize)>>,
}

impl GeneralEndpoint for CorpusEndpoint {
    fn model(&self) -> &str {
        self.transport.model()
    }

    fn complete_json(
        &self,
        prompt: &str,
        payload: &str,
        schema_name: &str,
        schema: &Value,
    ) -> Result<Value, GeneralEndpointError> {
        assert_eq!(schema_name, "entity_identity_comparison");
        assert!(
            payload.len() <= 16 * 1024,
            "model input exceeded the comparison budget"
        );
        let input: Value = serde_json::from_str(payload).unwrap();
        for (side, expected) in ["left", "right"].into_iter().zip(&self.quotes) {
            assert!(input[side]["relationships"].as_array().unwrap().is_empty());
            let mentions = input[side]["mentions"].as_array().unwrap();
            assert_eq!(mentions.len(), expected.len());
            assert!(
                mentions
                    .iter()
                    .zip(expected)
                    .all(|(m, q)| m["quote"].as_str() == Some(q)),
                "request did not carry the exact retained source excerpts"
            );
        }
        let mut once = self.observation.lock().unwrap();
        assert!(once.is_none(), "comparison repeated model inference");
        let output = self
            .transport
            .complete_json(prompt, payload, schema_name, schema)?;
        *once = Some((
            output["outcome"].as_str().unwrap().into(),
            output["reason"].as_str().unwrap().into(),
            output["citations"].as_array().unwrap().len(),
        ));
        Ok(output)
    }
}
