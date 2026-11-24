//! Cross-REL discovery through a live authenticated Gateway session.
use super::*;
use reliquary_application::entity_identity::IdentityDiscoveryIndex;
use reliquary_container::Container;
use reliquary_owner_session::RelSession;
use reliquary_perception_store::{EntityDraft, PRINCIPAL_ENTITY_KIND, PerceptionStore};
use std::collections::BTreeSet;

const LEFT: &str = "Kestrel entered the port registry under hull number M42-7816.";
const RIGHT: &str = "Horizon is the vessel bearing hull number M42-7816 in the survey.";

#[derive(Default)]
struct DiscoveryEndpoint {
    seen: Mutex<Vec<Value>>,
}

impl GeneralEndpoint for DiscoveryEndpoint {
    fn model(&self) -> &str {
        "discovery-fixture"
    }

    fn complete_json(
        &self,
        _: &str,
        payload: &str,
        schema: &str,
        _: &Value,
    ) -> Result<Value, GeneralEndpointError> {
        assert_eq!(schema, "entity_identity_comparison");
        let input: Value = serde_json::from_str(payload).unwrap();
        let witnessed_serial = ["left", "right"].map(|side| {
            input[side]["mentions"]
                .as_array()
                .unwrap()
                .iter()
                .enumerate()
                .find_map(|(index, row)| {
                    let quote = row["quote"].as_str().unwrap();
                    [LEFT, RIGHT]
                        .contains(&quote)
                        .then_some((index, quote.to_owned()))
                })
        });
        self.seen.lock().unwrap().push(input);
        if let [Some((left, left_quote)), Some((right, right_quote))] = witnessed_serial {
            return Ok(json!({
                "outcome": "same_identity",
                "reason": "corroborated",
                "citations": [
                    {"side": "left", "mention": left, "fact": left_quote, "bearing": "corroborates_identity"},
                    {"side": "right", "mention": right, "fact": right_quote, "bearing": "corroborates_identity"}
                ]
            }));
        }
        Ok(json!({"outcome": "unresolved", "reason": "ambiguity", "citations": []}))
    }
}

fn discovery_runtime(endpoint: Arc<DiscoveryEndpoint>) -> ConfiguredRuntime {
    ConfiguredRuntime::hosted(
        endpoint.clone(),
        Arc::new(UnusedEmbedding),
        HostedInferenceIdentity::new(
            "discovery-fixture",
            endpoint.model(),
            "fixture",
            "unused",
            2,
            HostedEmbeddingInputSemantics::Shared,
            EmbeddingNormalization::L2,
        )
        .unwrap(),
    )
    .unwrap()
}

fn seed_discovery(
    path: &Path,
    owner: [u8; 16],
    entities: &[(&str, &[&str], Option<&str>)],
) -> Vec<EntityId> {
    drop(RelWriter::create_current_path(path, owner).unwrap());
    let ids = {
        let mut container = Container::open_writer(path).unwrap();
        let mut perception = PerceptionStore::open(&container).unwrap();
        entities
            .iter()
            .map(|(name, aliases, _)| {
                perception
                    .publish_entity(
                        &mut container,
                        EntityDraft {
                            canonical_name: (*name).into(),
                            aliases: aliases.iter().map(|alias| (*alias).into()).collect(),
                            kind: "artifact".into(),
                            summary: "Fixture referent".into(),
                            mutation_id: (*name).into(),
                            created_at_ns: 10,
                            updated_at_ns: 10,
                        },
                    )
                    .unwrap()
                    .id
            })
            .collect::<Vec<_>>()
    };
    let mut writer = RelWriter::open_path_current(path).unwrap();
    for (&(name, _, quote), &entity_id) in entities.iter().zip(&ids) {
        let Some(quote) = quote else { continue };
        publish_witness(&mut writer, owner, entity_id, name, quote);
    }
    ids
}

fn publish_witness(
    writer: &mut RelWriter,
    owner: [u8; 16],
    entity_id: EntityId,
    name: &str,
    quote: &str,
) {
    let memory = writer
        .publish_direct_memory("author", draft(owner, name, quote))
        .unwrap()
        .record;
    let start_byte = quote.find(name).unwrap() as u32;
    writer
        .commit_entity_mention(EntityMentionCommit {
            owner_id: owner,
            memory_revision: memory.revision,
            body_id: memory.body_id,
            key: MemoryEntityMentionKey {
                memory_id: memory.id.0,
                field: MemoryTextField::Content,
                start_byte,
                end_byte: start_byte + name.len() as u32,
            },
            surface: name.into(),
            expected_revision: 0,
            updated_at_ns: 11,
            status: MemoryEntityResolutionStatus::Resolved {
                entity_id,
                reason: EntityResolutionReason::ContextMatch,
            },
            target_revisions: vec![(entity_id, 1)],
            new_entity: None,
        })
        .unwrap();
}

#[test]
fn authenticated_discovery_records_names_aliases_and_grounded_identifiers() {
    let dir = Directory::new();
    let ap = dir.0.join("a.rel");
    let bp = dir.0.join("b.rel");
    let cp = dir.0.join("ungranted.rel");
    let a_entities = seed_discovery(
        &ap,
        [51; 16],
        &[
            ("Blue Ledger", &[], None),
            ("North Gauge", &["Western Instrument"], None),
            ("Kestrel", &[], None),
        ],
    );
    let b_entities = seed_discovery(
        &bp,
        [52; 16],
        &[
            ("blue ledger", &[], None),
            ("Western Instrument", &[], None),
            ("Horizon", &[], Some(RIGHT)),
            (
                "Sandpiper",
                &[],
                Some("Sandpiper crossed the harbor with hull number Q91-4832."),
            ),
        ],
    );
    seed_discovery(
        &cp,
        [53; 16],
        &[(
            "Snowbird",
            &[],
            Some("The audit lists Snowbird under hull number M42-7816."),
        )],
    );
    let gateway = Gateway::new();
    let a = gateway
        .mount_owner_path(&ap, OwnerKind::Rel, true, None)
        .unwrap();
    let b = gateway
        .mount_owner_path(&bp, OwnerKind::Rel, false, None)
        .unwrap();
    let c = gateway
        .mount_owner_path(&cp, OwnerKind::Rel, false, None)
        .unwrap();
    let endpoint = Arc::new(DiscoveryEndpoint::default());
    let runtime = discovery_runtime(endpoint.clone());
    gateway.set_runtime(Some(&runtime)).unwrap();
    let token = "automatic-discovery-manager-credential-0000";
    let (admission, _) = TrustedHostAdmission::new([HostCredentialBinding::new(
        "author",
        token,
        [OwnerGrant::ManageRel(a), OwnerGrant::ViewRel(b)],
    )])
    .install(&gateway)
    .unwrap();
    let instance = admission.authenticate(token).unwrap();
    let work = instance.workspace_management(a).unwrap();

    let wait_for_count = |count| {
        let deadline = Instant::now() + Duration::from_secs(15);
        loop {
            let pairs = work.list_entity_identity_pairs(b, None, 10).unwrap();
            if pairs.len() >= count {
                break pairs;
            }
            assert!(
                Instant::now() < deadline,
                "automatic pair discovery timed out"
            );
            std::thread::sleep(Duration::from_millis(10));
        }
    };
    let initial = wait_for_count(2);
    assert!(
        initial
            .iter()
            .all(|pair| pair.local_entity != a_entities[2])
    );

    // Publish the left witness after the worker has already indexed these owners.
    // Keep the same authenticated session and let ordinary reopen refresh discovery.
    work.quiesce().unwrap();
    {
        let mut writer = RelWriter::open_path_current(&ap).unwrap();
        publish_witness(&mut writer, a.as_bytes(), a_entities[2], "Kestrel", LEFT);
    }
    work.reopen().unwrap();
    let pairs = wait_for_count(3);
    let discovered: BTreeSet<_> = pairs
        .iter()
        .map(|pair| (pair.local_entity, pair.peer_entity))
        .collect();
    assert_eq!(
        discovered,
        a_entities
            .iter()
            .copied()
            .zip(b_entities.iter().copied())
            .take(3)
            .collect()
    );
    let head = work.list_entity_identity_pairs(b, None, 1).unwrap();
    let cursor = (head[0].local_entity, head[0].peer_entity);
    assert_eq!(
        work.list_entity_identity_pairs(b, Some(cursor), 10)
            .unwrap(),
        pairs[1..]
    );
    for pair in pairs {
        let view = work
            .inspect_entity_identity(pair.local_entity, b, pair.peer_entity, None)
            .unwrap()
            .unwrap();
        assert_eq!(view.state, IdentityComparisonState::Current);
        assert_eq!(view.receipt.revision, 1);
        if pair.local_entity == a_entities[2] {
            assert_eq!(view.receipt.outcome, IdentityOutcome::SameIdentity);
            assert_eq!(view.receipt.citations.len(), 2);
            assert_eq!(view.cited_quotes.unwrap(), [LEFT, RIGHT]);
        } else {
            assert_eq!(view.receipt.outcome, IdentityOutcome::Unresolved);
            assert_eq!(view.receipt.reason, IdentityReason::InsufficientEvidence);
            assert!(view.receipt.citations.is_empty());
        }
    }
    assert_eq!(
        work.list_entity_identity_pairs(c, None, 10)
            .unwrap_err()
            .kind(),
        ProductErrorKind::Unauthorized
    );
    assert_eq!(endpoint.seen.lock().unwrap().len(), 1);
    work.quiesce().unwrap();
    let session = reliquary_owner_session::RelSession::open_path_current(&ap).unwrap();
    assert_eq!(
        session
            .identity_pairs_after(None, 10)
            .into_iter()
            .map(|pair| pair.peer_owner)
            .collect::<Vec<_>>(),
        [b.as_bytes(); 3]
    );
}

#[test]
#[ignore = "requires RELIQUARY_MENTION_CORPUS_DIR with the native 28-day and Ellis fixtures"]
fn authoritative_copies_discover_without_presupplied_entity_ids() {
    let root = PathBuf::from(std::env::var_os("RELIQUARY_MENTION_CORPUS_DIR").unwrap());
    let originals = [
        root.join("chatgpt-first28d/project.prj.rel"),
        root.join("rhelm-david-r-ellis-pre-relationship/project.prj.rel"),
    ];
    let digest = |path: &Path| {
        blake3::Hasher::new()
            .update_reader(fs::File::open(path).unwrap())
            .unwrap()
            .finalize()
    };
    let hashes = originals.each_ref().map(|path| digest(path));
    let dir = Directory::new();
    let copies = [dir.0.join("28d.rel"), dir.0.join("ellis.rel")];
    for ((source, copy), hash) in originals.iter().zip(&copies).zip(hashes) {
        fs::copy(source, copy).unwrap();
        assert_eq!(digest(copy), hash);
    }

    // Measure the coarse retrieval signals independently of opening authorized work.
    // A collision identifies evidence to compare and supplies no identity label.
    let indexes = copies.each_ref().map(|path| {
        let owner = RelSession::open_path_current(path).unwrap();
        let mut after = None;
        let mut index = IdentityDiscoveryIndex::default();
        loop {
            let page = owner.identity_discovery_keys(after, 64).unwrap();
            let Some((last, _)) = page.last() else { break };
            after = Some(*last);
            index.extend(page);
        }
        assert!(owner.identity_pairs_after(None, 1).is_empty());
        index
    });
    let [local_index, peer_index] = indexes;
    let mut after = None;
    let mut candidates = BTreeSet::new();
    loop {
        let page = local_index.candidate_rows(&peer_index, after, 64);
        let Some((last, _)) = page.last() else { break };
        after = Some(*last);
        for (local, peers) in page {
            candidates.extend(peers.into_iter().map(|peer| (local, peer)));
        }
    }
    println!(
        "28-day/Ellis: {} projected candidate pairs",
        candidates.len()
    );

    let gateway = Gateway::new();
    let local = gateway
        .mount_owner_path(&copies[0], OwnerKind::Rel, true, None)
        .unwrap();
    let peer = gateway
        .mount_owner_path(&copies[1], OwnerKind::Rel, false, None)
        .unwrap();
    let endpoint = Arc::new(DiscoveryEndpoint::default());
    gateway
        .set_runtime(Some(&discovery_runtime(endpoint.clone())))
        .unwrap();
    let token = "automatic-discovery-corpus-credential-000000";
    let (admission, _) = TrustedHostAdmission::new([HostCredentialBinding::new(
        "author",
        token,
        [OwnerGrant::ManageRel(local), OwnerGrant::ViewRel(peer)],
    )])
    .install(&gateway)
    .unwrap();
    let instance = admission.authenticate(token).unwrap();
    let work = instance.workspace_management(local).unwrap();
    let deadline = Instant::now() + Duration::from_secs(90);
    let pairs = loop {
        let mut pairs = Vec::new();
        let mut after = None;
        loop {
            let page = work.list_entity_identity_pairs(peer, after, 64).unwrap();
            let Some(last) = page.last() else { break };
            after = Some((last.local_entity, last.peer_entity));
            pairs.extend(page);
        }
        if pairs.len() == candidates.len() {
            break pairs;
        }
        assert!(Instant::now() < deadline, "no native candidate published");
        std::thread::sleep(Duration::from_millis(100));
    };

    for pair in &pairs {
        assert!(candidates.contains(&(pair.local_entity, pair.peer_entity)));
        let view = work
            .inspect_entity_identity(pair.local_entity, peer, pair.peer_entity, None)
            .unwrap()
            .unwrap();
        assert_eq!(view.state, IdentityComparisonState::Current);
        assert_eq!(view.receipt.outcome, IdentityOutcome::Unresolved);
    }
    work.quiesce().unwrap();
    let reopened = RelSession::open_path_current(&copies[0]).unwrap();
    let recorded = reopened.identity_pairs_after(None, usize::MAX);
    assert_eq!(pairs, recorded);
    let calls = endpoint.seen.lock().unwrap().len();
    println!(
        "{} durable receipts after reopen, {calls} local scripted evaluations",
        recorded.len()
    );
    assert_eq!(originals.each_ref().map(|path| digest(path)), hashes);
    assert_eq!(digest(&copies[1]), hashes[1]);
    if !candidates.is_empty() {
        return;
    }
    drop(reopened);

    // Separate positive control: add exactly one alias match on the disposable
    // recording copy. The matching Ellis Entity and all source files stay intact.
    let peer_entity = {
        let owner = RelSession::open_path_current(&copies[1]).unwrap();
        let entities = owner.entities();
        entities
            .iter()
            .filter(|entity| entity.kind != PRINCIPAL_ENTITY_KIND)
            .find(|entity| {
                let name = folded_label(&entity.canonical_name);
                !name.is_empty()
                    && entities.iter().all(|other| {
                        entity.id == other.id
                            || other.kind == PRINCIPAL_ENTITY_KIND
                            || std::iter::once(&other.canonical_name)
                                .chain(&other.aliases)
                                .all(|label| folded_label(label) != name)
                    })
            })
            .unwrap()
            .clone()
    };
    let control = {
        let mut container = Container::open_writer(&copies[0]).unwrap();
        let mut perception = PerceptionStore::open(&container).unwrap();
        perception
            .publish_entity(
                &mut container,
                EntityDraft {
                    canonical_name: format!("Offline discovery control {}", uuid::Uuid::new_v4()),
                    aliases: vec![peer_entity.canonical_name],
                    kind: peer_entity.kind,
                    summary: "Disposable discovery control".into(),
                    mutation_id: "offline-discovery-positive-control".into(),
                    created_at_ns: 10,
                    updated_at_ns: 10,
                },
            )
            .unwrap()
    };
    work.reopen().unwrap();
    let started = Instant::now();
    let pair = loop {
        let pairs = work.list_entity_identity_pairs(peer, None, 10).unwrap();
        if let [pair] = pairs.as_slice() {
            break *pair;
        }
        assert!(
            pairs.is_empty(),
            "only the added alias should nominate a pair"
        );
        assert!(
            started.elapsed() < Duration::from_secs(90),
            "seeded alias was not discovered"
        );
        std::thread::sleep(Duration::from_millis(100));
    };
    assert_eq!(pair.local_entity, control.id);
    assert_eq!(pair.peer_entity, peer_entity.id);
    let view = work
        .inspect_entity_identity(pair.local_entity, peer, pair.peer_entity, None)
        .unwrap()
        .unwrap();
    assert_eq!(view.state, IdentityComparisonState::Current);
    assert_eq!(view.receipt.outcome, IdentityOutcome::Unresolved);
    assert_eq!(view.receipt.reason, IdentityReason::InsufficientEvidence);
    assert!(view.receipt.left.mentions.is_empty() && view.receipt.left.relationships.is_empty());
    assert!(view.receipt.citations.is_empty());
    let discovery_time = started.elapsed();
    work.quiesce().unwrap();
    let owner = RelSession::open_path_current(&copies[0]).unwrap();
    assert_eq!(owner.identity_pairs_after(None, 2), [pair]);
    assert_eq!(owner.identity_receipt(&pair, None), Some(&view.receipt));
    assert!(endpoint.seen.lock().unwrap().is_empty());
    assert_eq!(originals.each_ref().map(|path| digest(path)), hashes);
    assert_eq!(digest(&copies[1]), hashes[1]);
    println!(
        "Separate disposable-copy control: one Unresolved/InsufficientEvidence receipt discovered in {:.2}s and verified after reopen, zero scripted evaluations",
        discovery_time.as_secs_f64()
    );
}

fn folded_label(label: &str) -> String {
    label
        .split_whitespace()
        .map(str::to_lowercase)
        .collect::<Vec<_>>()
        .join(" ")
}
