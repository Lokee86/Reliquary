use super::*;
use reliquary_memory::{IdentityCitation, IdentitySide, WithdrawEntityMentionRequest};

const LEFT: &str = "Alex Chen registered the vessel Kestrel under hull number M42-7816.";
const RIGHT: &str = "The port register lists Kestrel's hull number M42-7816 under Alex Chen.";

#[derive(Default)]
struct MentionScript(AtomicUsize);

impl GeneralEndpoint for MentionScript {
    fn model(&self) -> &str {
        "mention-identity-script"
    }

    fn complete_json(
        &self,
        _: &str,
        payload: &str,
        schema: &str,
        _: &Value,
    ) -> Result<Value, GeneralEndpointError> {
        assert_eq!(schema, "entity_identity_comparison");
        self.0.fetch_add(1, Ordering::SeqCst);
        let input: Value = serde_json::from_str(payload).unwrap();
        let citations = [("left", LEFT), ("right", RIGHT)].map(|(side, quote)| {
            assert_eq!(input[side]["relationships"], json!([]));
            let mentions = input[side]["mentions"].as_array().unwrap();
            let mention = mentions
                .iter()
                .position(|row| row["quote"] == quote)
                .unwrap();
            assert_eq!(mentions[mention]["surface"], PERSON);
            json!({
                "side": side,
                "mention": mention,
                "fact": quote,
                "bearing": "corroborates_identity",
            })
        });
        Ok(json!({
            "outcome": "same_identity",
            "reason": "corroborated",
            "citations": citations,
        }))
    }
}

fn append_context(
    path: &Path,
    owner: OwnerId,
    endpoint: EntityId,
    quote: &str,
) -> WithdrawEntityMentionRequest {
    let owner_id = owner.as_bytes();
    let mut writer = RelWriter::open_path_current(path).unwrap();
    let memory = writer
        .publish_direct_memory("author", draft(owner_id, "vessel-registration", quote))
        .unwrap()
        .record;
    let start_byte = quote.find(PERSON).unwrap();
    let key = MemoryEntityMentionKey {
        memory_id: memory.id.0,
        field: MemoryTextField::Content,
        start_byte: start_byte as u32,
        end_byte: (start_byte + PERSON.len()) as u32,
    };
    let resolution = writer
        .commit_entity_mention(EntityMentionCommit {
            owner_id,
            memory_revision: memory.revision,
            body_id: memory.body_id,
            key,
            surface: PERSON.into(),
            expected_revision: 0,
            updated_at_ns: 15,
            status: MemoryEntityResolutionStatus::Resolved {
                entity_id: endpoint,
                reason: EntityResolutionReason::ContextMatch,
            },
            target_revisions: vec![(endpoint, 1)],
            new_entity: None,
        })
        .unwrap();
    WithdrawEntityMentionRequest {
        owner_id,
        key,
        expected_mention_revision: resolution.revision,
        memory_id: memory.id,
        memory_revision: memory.revision,
        body_id: memory.body_id,
        source_surface: PERSON.into(),
        target_revisions: vec![(endpoint, 1)],
        withdrawn_at_ns: 16,
    }
}

#[test]
fn direct_mentions_reconsider_name_only_and_retain_historical_quotes() {
    let directory = Directory::new();
    let ap = directory.0.join("a.rel");
    let bp = directory.0.join("b.rel");
    let (ae, _) = seed(&ap, [43; 16]);
    let (be, _) = seed(&bp, [44; 16]);
    let gateway = Gateway::new();
    let a = gateway.mount_rel(&Rel::open(&ap).unwrap(), true).unwrap();
    let b = gateway.mount_rel(&Rel::open(&bp).unwrap(), true).unwrap();
    let token = "identity-mention-manager-credential-000000";
    let (admission, _) = TrustedHostAdmission::new([HostCredentialBinding::new(
        "author",
        token,
        [OwnerGrant::ManageRel(a), OwnerGrant::ManageRel(b)],
    )])
    .install(&gateway)
    .unwrap();
    let instance = admission.authenticate(token).unwrap();
    let work = instance.workspace_management(a).unwrap();
    let peer = instance.workspace_management(b).unwrap();
    let script = Arc::new(MentionScript::default());
    let runtime = ConfiguredRuntime::hosted(
        script.clone(),
        Arc::new(UnusedEmbedding),
        HostedInferenceIdentity::new(
            "mention-fixture",
            "mention-identity-script",
            "fixture",
            "unused",
            2,
            HostedEmbeddingInputSemantics::Shared,
            EmbeddingNormalization::L2,
        )
        .unwrap(),
    )
    .unwrap();
    gateway.set_runtime(Some(&runtime)).unwrap();

    let first = work.compare_entity_identity(ae, b, be).unwrap();
    assert_eq!(first.revision, 1);
    assert_eq!(first.outcome, IdentityOutcome::Unresolved);
    assert_eq!(first.reason, IdentityReason::InsufficientEvidence);
    assert!(first.citations.is_empty());
    assert_eq!(script.0.load(Ordering::SeqCst), 0);

    work.quiesce().unwrap();
    peer.quiesce().unwrap();
    append_context(&ap, a, ae, LEFT);
    let withdrawal = append_context(&bp, b, be, RIGHT);
    work.reopen().unwrap();
    peer.reopen().unwrap();

    wait_revision(&work, ae, b, be, 2);
    let inspect = |revision| {
        work.inspect_entity_identity(ae, b, be, revision)
            .unwrap()
            .unwrap()
    };
    let current = inspect(None);
    assert_eq!(current.receipt.predecessor, Some(first.revision));
    assert_eq!(current.receipt.outcome, IdentityOutcome::SameIdentity);
    assert!(current.receipt.left.relationships.is_empty());
    assert!(current.receipt.right.relationships.is_empty());
    assert!(matches!(
        current.receipt.citations.as_slice(),
        [
            IdentityCitation::Mention {
                side: IdentitySide::Left,
                ..
            },
            IdentityCitation::Mention {
                side: IdentitySide::Right,
                ..
            },
        ]
    ));
    assert_eq!(current.cited_quotes, Some(vec![LEFT.into(), RIGHT.into()]));
    assert_eq!(
        work.compare_entity_identity(ae, b, be).unwrap(),
        current.receipt
    );
    assert_eq!(script.0.load(Ordering::SeqCst), 1);

    peer.withdraw_entity_mention(withdrawal).unwrap();
    wait_revision(&work, ae, b, be, 3);
    work.quiesce().unwrap();
    work.reopen().unwrap();
    assert_eq!(inspect(None).receipt.outcome, IdentityOutcome::Unresolved);

    let historical = inspect(Some(2));
    assert_eq!(historical.state, IdentityComparisonState::Historical);
    assert_eq!(historical.receipt, current.receipt);
    assert_eq!(historical.cited_quotes, current.cited_quotes);
    assert_eq!(script.0.load(Ordering::SeqCst), 1);
}
