use crate::scheduler::Scheduler;
use crate::*;
use reliquary_container::Container;
use sha2::{Digest, Sha256};
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

static NEXT_TEST_FILE: AtomicU64 = AtomicU64::new(0);

fn episode(n: u8) -> EpisodeId {
    let mut b = [0; 32];
    b[31] = n;
    EpisodeId(b)
}
fn work(id: EpisodeId, state: InsomniaWorkState) -> InsomniaWork {
    InsomniaWork {
        episode_id: id,
        priority: InsomniaPriority::Live,
        state,
        attempt_count: 0,
        lease_owner: None,
        lease_token: None,
        lease_expires_ns: None,
        retry_after_ns: None,
        last_error: None,
        updated_at_ns: 0,
    }
}
fn order(id: EpisodeId, source: i64) -> AcceptedEpisodeOrder {
    AcceptedEpisodeOrder {
        episode_id: id,
        source_through_ns: source,
        conversation_id: "c".into(),
        start_node_id: "n".into(),
    }
}
fn completion(id: EpisodeId) -> AcceptedCompletionState {
    AcceptedCompletionState {
        episode_id: id,
        attempt: 1,
        started_at_ns: 10,
        completed_at_ns: 20,
        extractor_model: "m".into(),
        extractor_version: "v".into(),
        memory_ids: vec![MemoryId([4; 32])],
        external_memory_refs: vec![ExternalMemoryRef {
            owner_id: "phy".into(),
            memory_id: MemoryId([5; 32]),
        }],
        rejected_count: 0,
    }
}
fn container() -> (Container, PathBuf) {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../.test-tmp");
    fs::create_dir_all(&root).unwrap();
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let sequence = NEXT_TEST_FILE.fetch_add(1, Ordering::Relaxed);
    let path = root.join(format!(
        "insomnia-store-{}-{stamp}-{sequence}.cva",
        std::process::id()
    ));
    (Container::create(&path).unwrap(), path)
}
#[test]
fn format_work_and_attempt_codecs_preserve_current_wire_bytes() {
    assert_eq!(codec::encode_format(), b"CVAINSF1");
    let w = work(episode(7), InsomniaWorkState::Pending);
    let bytes = codec::encode_work(&w).unwrap();
    let mut expected = b"CVAINSW1".to_vec();
    expected.extend_from_slice(&episode(7).0);
    expected.extend_from_slice(&[2, 1]);
    expected.extend_from_slice(&0u32.to_le_bytes());
    expected.extend_from_slice(&0i64.to_le_bytes());
    expected.extend_from_slice(&[0, 0, 0, 0, 0]);
    assert_eq!(bytes, expected);
    assert!(
        matches!(codec::decode_record(&bytes).unwrap(),Some(codec::InsomniaRecord::Work(decoded)) if decoded==w)
    );
    let a = InsomniaAttempt {
        episode_id: episode(8),
        attempt: 2,
        state: InsomniaWorkState::Complete,
        started_at_ns: 11,
        completed_at_ns: 12,
        extractor_model: "m".into(),
        extractor_version: "v".into(),
        memory_ids: vec![MemoryId([9; 32])],
        external_memory_refs: vec![],
        rejected_count: 3,
        error: None,
    };
    let ab = codec::encode_attempt(&a).unwrap();
    let mut ae = b"CVAINSA1".to_vec();
    ae.extend_from_slice(&episode(8).0);
    ae.extend_from_slice(&2u32.to_le_bytes());
    ae.push(3);
    ae.extend_from_slice(&11i64.to_le_bytes());
    ae.extend_from_slice(&12i64.to_le_bytes());
    ae.extend_from_slice(&3u32.to_le_bytes());
    ae.extend_from_slice(&1u32.to_le_bytes());
    ae.push(b'm');
    ae.extend_from_slice(&1u32.to_le_bytes());
    ae.push(b'v');
    ae.extend_from_slice(&1u32.to_le_bytes());
    ae.extend_from_slice(&[9; 32]);
    ae.push(0);
    assert_eq!(ab, ae);
    let decoded = match codec::decode_record(&ab).unwrap().unwrap() {
        codec::InsomniaRecord::Attempt(a) => a,
        _ => panic!("wrong record"),
    };
    assert_eq!(decoded, a);
    assert!(codec::decode_format(b"CVAINSF1x").is_err());
}
#[test]
fn scheduler_uses_priority_then_authenticated_source_order() {
    let mut s = Scheduler::default();
    let low = work(episode(2), InsomniaWorkState::Pending);
    let urgent = InsomniaWork {
        priority: InsomniaPriority::ImmediateLive,
        ..work(episode(9), InsomniaWorkState::Pending)
    };
    s.register(&order(episode(2), 1), &low);
    s.register(&order(episode(9), 50), &urgent);
    assert_eq!(s.next_ready(0), Some(episode(9)));
}
#[test]
fn failure_retry_lease_expiry_and_renewal_transitions_match_state_rules() {
    let (mut c, p) = container();
    let id = episode(3);
    let mut s = InsomniaStore::empty();
    s.queue(&mut c, order(id, 1), InsomniaPriority::Live, 1)
        .unwrap();
    let claimed = s.claim_next(&mut c, " worker ", 2, 10).unwrap().unwrap();
    assert_eq!(claimed.lease_owner.as_deref(), Some("worker"));
    let mut hash = Sha256::new();
    hash.update(b"continuity-insomnia-lease\0");
    hash.update(id.0);
    hash.update(8u64.to_le_bytes());
    hash.update(b" worker ");
    hash.update(1u32.to_le_bytes());
    hash.update(2i64.to_le_bytes());
    assert_eq!(
        claimed.lease_token,
        Some(InsomniaLeaseToken(hash.finalize().into()))
    );
    let renewed = s
        .renew(&mut c, id, claimed.lease_token.unwrap(), 5, 10)
        .unwrap();
    assert_eq!(renewed.lease_expires_ns, Some(15));
    assert!(matches!(
        s.renew(&mut c, id, claimed.lease_token.unwrap(), 15, 10),
        Err(InsomniaError::LeaseExpired)
    ));
    let expired_claim = s.claim_next(&mut c, "next", 15, 10).unwrap().unwrap();
    let failed = s
        .fail(
            &mut c,
            id,
            expired_claim.lease_token.unwrap(),
            15,
            16,
            30,
            "transient".into(),
        )
        .unwrap();
    assert_eq!(failed.state, InsomniaWorkState::Failed);
    assert_eq!(s.next_ready(29), None);
    assert_eq!(s.next_ready(30), Some(id));
    let retry = s.claim_next(&mut c, "next", 30, 10).unwrap().unwrap();
    assert_eq!(retry.attempt_count, 3);
    drop(c);
    fs::remove_file(p).unwrap();
}
#[test]
fn ephemeral_transitions_do_not_append_but_terminal_retry_do() {
    let (mut c, p) = container();
    let mut s = InsomniaStore::empty();
    let id = episode(4);
    s.initialize(&mut c).unwrap();
    let base = c.chunk_refs().unwrap().len();
    s.queue(&mut c, order(id, 1), InsomniaPriority::Live, 1)
        .unwrap();
    let claim = s.claim_next(&mut c, "w", 2, 10).unwrap().unwrap();
    s.renew(&mut c, id, claim.lease_token.unwrap(), 3, 10)
        .unwrap();
    assert_eq!(c.chunk_refs().unwrap().len(), base);
    s.terminal(&mut c, id, claim.lease_token.unwrap(), 2, 4, "stop".into())
        .unwrap();
    assert_eq!(c.chunk_refs().unwrap().len(), base + 1);
    s.retry_terminal(&mut c, id, 5).unwrap();
    assert_eq!(c.chunk_refs().unwrap().len(), base + 2);
    drop(c);
    fs::remove_file(p).unwrap();
}
#[test]
fn replay_applies_physical_work_and_attempt_order() {
    let mut s = InsomniaStore::empty();
    let id = episode(6);
    let first = work(id, InsomniaWorkState::Pending);
    let second = InsomniaWork {
        state: InsomniaWorkState::Terminal,
        attempt_count: 2,
        ..first.clone()
    };
    let raw_attempt = InsomniaAttempt {
        episode_id: id,
        attempt: 2,
        state: InsomniaWorkState::Failed,
        started_at_ns: 1,
        completed_at_ns: 2,
        extractor_model: "m".into(),
        extractor_version: "v".into(),
        memory_ids: vec![],
        external_memory_refs: vec![],
        rejected_count: 0,
        error: Some("e".into()),
    };
    s.replay_record(&codec::encode_work(&first).unwrap())
        .unwrap();
    s.replay_record(&codec::encode_work(&second).unwrap())
        .unwrap();
    s.replay_record(&codec::encode_attempt(&raw_attempt).unwrap())
        .unwrap();
    assert_eq!(s.work(id), Some(&second));
    assert_eq!(s.attempts(id), vec![raw_attempt]);
}
#[test]
fn completion_projection_preserves_same_attempt_replacement_semantics() {
    let mut s = InsomniaStore::empty();
    let id = episode(10);
    s.apply_work(work(id, InsomniaWorkState::Processing));
    let c = completion(id);
    assert!(matches!(s.check_completion(&c), Ok(CompletionCheck::Apply)));
    assert!(s.attempts(id).is_empty());
    s.apply_completion(&c).unwrap();
    let after_first = s.attempts(id);
    assert!(matches!(
        s.check_completion(&c),
        Ok(CompletionCheck::AlreadyAccepted)
    ));
    let after_first_work = s.work(id).cloned();
    s.apply_completion(&c).unwrap();
    assert_eq!(s.attempts(id), after_first);
    assert_eq!(s.work(id), after_first_work.as_ref());
    let mut conflicting = c.clone();
    conflicting.extractor_model = "other".into();
    assert!(matches!(
        s.check_completion(&conflicting),
        Err(InsomniaError::InvalidTransition)
    ));
    assert!(matches!(
        s.apply_completion(&conflicting),
        Err(InsomniaError::InvalidTransition)
    ));
    assert_eq!(s.attempts(id).len(), 1);
    assert_eq!(s.attempts(id)[0].extractor_model, "m");
    assert_eq!(s.work(id), after_first_work.as_ref());
}

#[test]
fn completion_preflight_allows_a_new_progressed_attempt_without_mutation() {
    let mut s = InsomniaStore::empty();
    let id = episode(16);
    let mut prior = completion(id);
    prior.attempt = 1;
    s.apply_completion(&prior).unwrap();
    s.apply_work(InsomniaWork {
        state: InsomniaWorkState::Processing,
        attempt_count: 2,
        lease_owner: Some("worker".into()),
        lease_token: Some(InsomniaLeaseToken([6; 32])),
        lease_expires_ns: Some(100),
        ..work(id, InsomniaWorkState::Processing)
    });
    let before_work = s.work(id).cloned();
    let before_attempts = s.attempts(id);
    let mut next = completion(id);
    next.attempt = 2;
    next.started_at_ns = 30;
    next.completed_at_ns = 40;

    assert!(matches!(
        s.check_completion(&next),
        Ok(CompletionCheck::Apply)
    ));
    assert_eq!(s.work(id), before_work.as_ref());
    assert_eq!(s.attempts(id), before_attempts);
    s.apply_completion(&next).unwrap();
    assert_eq!(s.work(id).unwrap().attempt_count, 2);
    assert_eq!(s.attempts(id).len(), 1);
    assert_eq!(s.attempts(id)[0].attempt, 2);
}

#[test]
fn exact_duplicate_reapplies_after_intervening_work_in_physical_order() {
    let mut s = InsomniaStore::empty();
    let id = episode(17);
    let c = completion(id);
    s.apply_completion(&c).unwrap(); // receipt arrives before work
    s.apply_work(InsomniaWork {
        state: InsomniaWorkState::Terminal,
        attempt_count: 1,
        last_error: Some("later terminal work".into()),
        ..work(id, InsomniaWorkState::Terminal)
    });

    // Since the intervening work record changed the projection, replaying the
    // same receipt must apply it again instead of returning early.
    assert!(matches!(s.check_completion(&c), Ok(CompletionCheck::Apply)));
    s.apply_completion(&c).unwrap();
    assert_eq!(s.work(id).unwrap().state, InsomniaWorkState::Complete);
    assert_eq!(s.work(id).unwrap().attempt_count, c.attempt);
    assert_eq!(s.attempts(id).len(), 1);

    let stale_id = episode(18);
    let stale = completion(stale_id);
    s.apply_completion(&stale).unwrap();
    s.apply_work(InsomniaWork {
        state: InsomniaWorkState::Processing,
        attempt_count: 2,
        ..work(stale_id, InsomniaWorkState::Processing)
    });
    assert!(matches!(
        s.check_completion(&stale),
        Err(InsomniaError::InvalidTransition)
    ));
}

#[test]
fn malformed_attempt_count_is_rejected_before_allocation() {
    let mut bytes = b"CVAINSA1".to_vec();
    bytes.extend_from_slice(&episode(11).0);
    bytes.extend_from_slice(&1u32.to_le_bytes());
    bytes.push(InsomniaWorkState::Complete.tag());
    bytes.extend_from_slice(&0i64.to_le_bytes());
    bytes.extend_from_slice(&1i64.to_le_bytes());
    bytes.extend_from_slice(&0u32.to_le_bytes());
    bytes.extend_from_slice(&0u32.to_le_bytes()); // model
    bytes.extend_from_slice(&0u32.to_le_bytes()); // version
    bytes.extend_from_slice(&u32::MAX.to_le_bytes()); // memory id count

    assert!(matches!(
        codec::decode_record(&bytes),
        Err(InsomniaError::CorruptRecord("truncated memory id"))
    ));
}

#[test]
fn lower_codecs_reject_truncated_and_foreign_family_versions() {
    for bytes in [b"CVAINSF".as_slice(), b"CVAINSF2"] {
        assert!(matches!(
            codec::decode_format(bytes),
            Err(InsomniaError::CorruptRecord(_))
        ));
    }
    for bytes in [b"CVAINSW".as_slice(), b"CVAINSW2", b"CVAINSA", b"CVAINSA2"] {
        assert!(matches!(
            codec::decode_record(bytes),
            Err(InsomniaError::CorruptRecord(_))
        ));
    }

    // Full completion envelopes belong to the higher Memory/Freshness-aware
    // decoder and must remain opaque to this lower work/attempt codec.
    assert!(codec::decode_record(b"CVAINSC7").unwrap().is_none());
    assert!(codec::decode_record(b"CVAINSC8").unwrap().is_none());
    assert!(!codec::decode_format(b"CVAINSC7").unwrap());
}

fn accepted_state(
    id: EpisodeId,
    priority: InsomniaPriority,
    finalized_at_ns: i64,
) -> AcceptedEpisodeState {
    AcceptedEpisodeState {
        order: order(id, finalized_at_ns),
        priority,
        finalized_at_ns,
    }
}

#[test]
fn rebuild_prefers_completion_then_final_work_then_fresh_pending() {
    let mut s = InsomniaStore::empty();
    let completed_id = episode(12);
    let terminal_id = episode(13);
    let transient_id = episode(14);
    let missing_id = episode(15);
    s.apply_work(InsomniaWork {
        state: InsomniaWorkState::Terminal,
        attempt_count: 9,
        lease_owner: Some("stale".into()),
        lease_token: Some(InsomniaLeaseToken([1; 32])),
        lease_expires_ns: Some(99),
        retry_after_ns: Some(100),
        last_error: Some("terminal".into()),
        ..work(completed_id, InsomniaWorkState::Terminal)
    });
    s.apply_work(InsomniaWork {
        state: InsomniaWorkState::Terminal,
        attempt_count: 4,
        lease_owner: Some("stale".into()),
        lease_token: Some(InsomniaLeaseToken([2; 32])),
        lease_expires_ns: Some(99),
        retry_after_ns: Some(100),
        last_error: Some("terminal".into()),
        ..work(terminal_id, InsomniaWorkState::Terminal)
    });
    s.apply_work(InsomniaWork {
        state: InsomniaWorkState::Processing,
        attempt_count: 7,
        lease_owner: Some("stale".into()),
        lease_token: Some(InsomniaLeaseToken([3; 32])),
        lease_expires_ns: Some(99),
        ..work(transient_id, InsomniaWorkState::Processing)
    });
    s.apply_attempt(InsomniaAttempt {
        episode_id: completed_id,
        attempt: 3,
        state: InsomniaWorkState::Complete,
        started_at_ns: 20,
        completed_at_ns: 30,
        extractor_model: "m".into(),
        extractor_version: "v".into(),
        memory_ids: vec![],
        external_memory_refs: vec![],
        rejected_count: 0,
        error: None,
    });

    s.rebuild_episodes(&[
        accepted_state(completed_id, InsomniaPriority::ImmediateLive, 10),
        accepted_state(terminal_id, InsomniaPriority::Import, 11),
        accepted_state(transient_id, InsomniaPriority::Live, 12),
        accepted_state(missing_id, InsomniaPriority::Live, 13),
    ]);

    let completed = s.work(completed_id).unwrap();
    assert_eq!(completed.state, InsomniaWorkState::Complete);
    assert_eq!(completed.attempt_count, 3);
    assert_eq!(completed.priority, InsomniaPriority::ImmediateLive);
    assert_eq!(completed.updated_at_ns, 30);
    assert!(completed.lease_token.is_none());
    let terminal = s.work(terminal_id).unwrap();
    assert_eq!(terminal.state, InsomniaWorkState::Terminal);
    assert_eq!(terminal.attempt_count, 4);
    assert_eq!(terminal.priority, InsomniaPriority::Import);
    assert!(terminal.lease_token.is_none());
    assert!(terminal.retry_after_ns.is_none());
    let transient = s.work(transient_id).unwrap();
    assert_eq!(transient.state, InsomniaWorkState::Pending);
    assert_eq!(transient.attempt_count, 0);
    assert_eq!(transient.updated_at_ns, 12);
    assert_eq!(s.work(missing_id).unwrap().updated_at_ns, 13);
    assert!(s.work(episode(99)).is_none());
    assert_eq!(s.stats().total, 4);
    assert_eq!(s.stats().complete, 1);
    assert_eq!(s.stats().terminal, 1);
    assert_eq!(s.stats().pending, 2);
    assert_eq!(s.next_ready(12), Some(transient_id));
}
