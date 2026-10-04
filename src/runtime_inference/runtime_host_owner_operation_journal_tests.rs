use super::*;
use std::{
    fs,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};
fn temp() -> PathBuf {
    std::env::temp_dir().join(format!(
        "rqop-{}-{}",
        Uuid::new_v4(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ))
}
fn attr(id: &str) -> Attribution {
    Attribution {
        generation_id: id.into(),
        conversation_id: "c1".into(),
        submission_id: "s1".into(),
        actor_class: ActorClass::User,
        actor_principal: Some("pA".into()),
        generation_initiator: Some("pA".into()),
        initiating_generation_id: None,
        admission: AdmissionIdentity {
            instance_id: "i1".into(),
            principal_id: Some("pA".into()),
            phy_owner_id: Some("phy-a".into()),
            personal: Some(PersonalAdmission {
                mount_epoch: 5,
                grant_generation: 2,
            }),
            mount_epoch: 3,
            grant_generation: 7,
        },
    }
}
fn plan(id: &str) -> EpisodePlan {
    EpisodePlan {
        episode_id: id.into(),
        attempt: 1,
        source_fingerprint: "fp-1".into(),
        expected_receipt_id: format!("receipt-{id}"),
        prepared: FrozenPreparedApplication {
            project_drafts: vec![],
            personal_drafts: vec![FrozenPersonalDraft {
                prepared_memory: FrozenPreparedMemory {
                    prepared_memory: serde_json::json!({"draft":{"title":"t","content":"body"},"temporal":{"source_time_ns":null},"routing":null}),
                },
                source_ref: serde_json::json!({"owner_id":"rel-x","principal_id":"pA","source_node_id":"n1"}),
                mutation_id: format!("mut-{id}"),
                destination_principal: "pA".into(),
                destination_phy_owner_id: "phy-a".into(),
                authorizing_generations: vec!["g1".into()],
            }],
            rejected: vec![],
            model: "model".into(),
        },
    }
}

#[test]
fn agent_sources_keep_original_generation_across_restart_and_compaction() {
    let root = temp();
    let rel = Uuid::new_v4().to_string();
    let mut j = OwnerOperationJournal::open(&root, &rel).unwrap();
    j.append_attribution(attr("user-a")).unwrap();
    let mut b = attr("user-b");
    b.submission_id = "user-b-node".into();
    b.actor_principal = Some("pB".into());
    b.generation_initiator = Some("pB".into());
    b.admission.principal_id = Some("pB".into());
    b.admission.phy_owner_id = Some("phy-b".into());
    b.admission.instance_id = "i-b".into();
    j.append_attribution(b).unwrap();
    let sequence = j
        .bind_agent_source("user-a", "agent-a", "checkpoint-a")
        .unwrap();
    assert_eq!(
        j.bind_agent_source("user-a", "agent-a", "checkpoint-a")
            .unwrap(),
        sequence
    );
    assert!(
        j.bind_agent_source("user-b", "agent-a", "checkpoint-a")
            .is_err()
    );
    assert!(
        j.bind_agent_source("agent-a", "nested-agent", "nested")
            .is_err()
    );
    assert!(
        j.bind_agent_source("missing", "unknown-agent", "unknown")
            .is_err()
    );
    j.compact().unwrap();
    drop(j);
    let j = OwnerOperationJournal::open(&root, &rel).unwrap();
    let agent = j
        .source_attribution("c1", "checkpoint-a", ActorClass::Agent)
        .unwrap();
    assert_eq!(agent.actor_principal, None);
    assert_eq!(agent.generation_initiator.as_deref(), Some("pA"));
    assert_eq!(agent.initiating_generation_id.as_deref(), Some("user-a"));
    assert_eq!(agent.admission, attr("user-a").admission);
    assert!(
        j.source_attribution("c1", "checkpoint-a", ActorClass::User)
            .is_none()
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn attribution_rejects_forged_user_and_agent_admissions() {
    let root = temp();
    let rel = Uuid::new_v4().to_string();
    let mut j = OwnerOperationJournal::open(&root, &rel).unwrap();
    let mut user = attr("bad-user");
    user.admission.principal_id = Some("pB".into());
    assert!(j.append_attribution(user).is_err());
    j.append_attribution(attr("user")).unwrap();
    j.bind_agent_source("user", "agent", "agent-node").unwrap();
    let mut forged = j.attribution("agent").unwrap().clone();
    forged.generation_id = "forged".into();
    forged.submission_id = "forged-node".into();
    forged.admission.phy_owner_id = Some("phy-b".into());
    assert!(j.append_attribution(forged.clone()).is_err());
    // Even a complete, correctly checksummed forged frame fails replay.
    let path = j.path.clone();
    let sequence = j.last_sequence() + 1;
    drop(j);
    let bytes = encode_frame(sequence, &OwnerOperationRecord::Attribution(forged)).unwrap();
    OpenOptions::new()
        .append(true)
        .open(&path)
        .unwrap()
        .write_all(&bytes)
        .unwrap();
    assert!(matches!(
        OwnerOperationJournal::open(&root, &rel),
        Err(JournalError::Corrupt(_))
    ));
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn source_role_conflicts_remain_unknown() {
    let root = temp();
    let rel = Uuid::new_v4().to_string();
    let mut j = OwnerOperationJournal::open(&root, &rel).unwrap();
    j.append_attribution(attr("user")).unwrap();
    // A duplicated source identity must not become authoritative by filtering
    // away the contradictory actor class.
    j.bind_agent_source("user", "agent", "s1").unwrap();
    assert!(j.source_attribution("c1", "s1", ActorClass::User).is_none());
    assert!(
        j.source_attribution("c1", "s1", ActorClass::Agent)
            .is_none()
    );
    j.compact().unwrap();
    drop(j);
    let j = OwnerOperationJournal::open(&root, &rel).unwrap();
    assert!(j.source_attribution("c1", "s1", ActorClass::User).is_none());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn revocation_keeps_original_agent_authority_withdrawn_after_compaction() {
    let root = temp();
    let rel = Uuid::new_v4().to_string();
    let mut journal = OwnerOperationJournal::open(&root, &rel).unwrap();
    journal.append_attribution(attr("user")).unwrap();
    journal
        .bind_agent_source("user", "agent", "agent-node")
        .unwrap();
    journal
        .revoke_personal_authority("other-instance", true, None)
        .unwrap();
    assert!(!journal.authority_revoked(journal.attribution("agent").unwrap()));
    journal
        .revoke_personal_authority("i1", false, Some("phy-a"))
        .unwrap();
    assert!(journal.authority_revoked(journal.attribution("agent").unwrap()));
    journal
        .append_held(Held {
            operation_id: "user".into(),
            reason: "later unrelated hold".into(),
        })
        .unwrap();
    journal.compact().unwrap();
    drop(journal);
    let journal = OwnerOperationJournal::open(&root, &rel).unwrap();
    assert!(journal.authority_revoked(journal.attribution("user").unwrap()));
    assert!(journal.authority_revoked(journal.attribution("agent").unwrap()));
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn append_reopen_and_exact_frozen_payload() {
    let root = temp();
    let rel = Uuid::new_v4().to_string();
    {
        let mut j = OwnerOperationJournal::open(&root, &rel).unwrap();
        assert_eq!(j.append_attribution(attr("g1")).unwrap(), 1);
        assert_eq!(j.append_episode_plan(plan("e1")).unwrap(), 2);
        assert_eq!(
            j.append_receipt_observed(EpisodeReceiptObserved {
                episode_id: "e1".into(),
                receipt_id: "receipt-e1".into(),
                verified_external_refs: vec![OwnerMemoryRef {
                    owner_id: "phy-a".into(),
                    memory_id: "m1".into()
                }]
            })
            .unwrap(),
            3
        );
    }
    let j = OwnerOperationJournal::open(&root, &rel).unwrap();
    assert_eq!(j.last_sequence(), 3);
    assert_eq!(j.attribution("g1"), Some(&attr("g1")));
    assert_eq!(j.episode_plan("e1"), Some(&plan("e1")));
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn immutable_attribution_plan_and_idempotent_reappend() {
    let root = temp();
    let rel = Uuid::new_v4().to_string();
    let mut j = OwnerOperationJournal::open(&root, &rel).unwrap();
    assert_eq!(j.append_attribution(attr("g1")).unwrap(), 1);
    assert_eq!(j.append_attribution(attr("g1")).unwrap(), 1);
    let mut a = attr("g1");
    a.generation_initiator = Some("pB".into());
    assert!(j.append_attribution(a).is_err());
    assert_eq!(j.append_episode_plan(plan("e1")).unwrap(), 2);
    let mut p = plan("e1");
    p.attempt = 2;
    assert!(j.append_episode_plan(p).is_err());
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn truncated_tail_repairs_but_complete_checksum_corruption_fails() {
    let root = temp();
    let rel = Uuid::new_v4().to_string();
    let mut j = OwnerOperationJournal::open(&root, &rel).unwrap();
    j.append_attribution(attr("g1")).unwrap();
    let path = j.path.clone();
    let len = fs::metadata(&path).unwrap().len();
    {
        let mut f = OpenOptions::new().append(true).open(&path).unwrap();
        f.write_all(&[0x10, 0, 0]).unwrap();
        f.sync_all().unwrap();
    }
    drop(j);
    let j = OwnerOperationJournal::open(&root, &rel).unwrap();
    assert_eq!(fs::metadata(&path).unwrap().len(), len);
    drop(j);
    let mut b = fs::read(&path).unwrap();
    let i = b.len() - 1;
    b[i] ^= 0x20;
    fs::write(&path, b).unwrap();
    assert!(matches!(
        OwnerOperationJournal::open(&root, &rel),
        Err(JournalError::Corrupt("frame checksum mismatch"))
    ));
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn wrong_rel_and_interior_corruption_fail_closed() {
    let root = temp();
    let rel = Uuid::new_v4().to_string();
    let mut j = OwnerOperationJournal::open(&root, &rel).unwrap();
    j.append_attribution(attr("g1")).unwrap();
    j.append_episode_plan(plan("e1")).unwrap();
    let path = j.path.clone();
    drop(j);
    let mut b = fs::read(&path).unwrap();
    b[HEADER_LEN as usize + PREFIX_LEN + 2] ^= 1;
    fs::write(&path, b).unwrap();
    assert!(OwnerOperationJournal::open(&root, &rel).is_err());
    let other_rel = Uuid::new_v4().to_string();
    fs::copy(
        &path,
        root.join("owner-operations")
            .join(format!("{other_rel}.roj")),
    )
    .unwrap();
    assert!(matches!(
        OwnerOperationJournal::open(&root, &other_rel),
        Err(JournalError::WrongRel { .. })
    ));
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn receipt_needs_frozen_plan_and_cannot_conflict() {
    let root = temp();
    let rel = Uuid::new_v4().to_string();
    let mut j = OwnerOperationJournal::open(&root, &rel).unwrap();
    let r = EpisodeReceiptObserved {
        episode_id: "e1".into(),
        receipt_id: "receipt-e1".into(),
        verified_external_refs: vec![OwnerMemoryRef {
            owner_id: "phy-a".into(),
            memory_id: "m1".into(),
        }],
    };
    assert!(j.append_receipt_observed(r.clone()).is_err());
    j.append_episode_plan(plan("e1")).unwrap();
    j.append_receipt_observed(r).unwrap();
    assert!(
        j.append_receipt_observed(EpisodeReceiptObserved {
            episode_id: "e1".into(),
            receipt_id: "different".into(),
            verified_external_refs: vec![]
        })
        .is_err()
    );
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn compaction_keeps_unresolved_plan_and_sequence_and_discards_receipted_plan() {
    let root = temp();
    let rel = Uuid::new_v4().to_string();
    let mut j = OwnerOperationJournal::open(&root, &rel).unwrap();
    j.append_attribution(attr("g1")).unwrap();
    j.append_episode_plan(plan("e1")).unwrap();
    j.append_episode_plan(plan("e2")).unwrap();
    j.append_receipt_observed(EpisodeReceiptObserved {
        episode_id: "e1".into(),
        receipt_id: "receipt-e1".into(),
        verified_external_refs: vec![OwnerMemoryRef {
            owner_id: "phy-a".into(),
            memory_id: "m1".into(),
        }],
    })
    .unwrap();
    let seq = j.last_sequence();
    j.compact().unwrap();
    drop(j);
    let mut j = OwnerOperationJournal::open(&root, &rel).unwrap();
    assert_eq!(j.last_sequence(), seq);
    assert!(j.episode_plan("e1").is_none());
    assert!(j.episode_plan("e2").is_some());
    assert_eq!(j.append_attribution(attr("g2")).unwrap(), seq + 1);
    drop(j);
    assert_eq!(
        OwnerOperationJournal::open(&root, &rel)
            .unwrap()
            .last_sequence(),
        seq + 1
    );
    fs::remove_dir_all(root).unwrap();
}
