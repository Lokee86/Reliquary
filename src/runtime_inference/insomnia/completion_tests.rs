use super::completion::decode_completion;

#[test]
fn freshness_grouped_birth_recovers_from_completion_without_appending() {
    let (path, mut cva, _) = setup("freshness-grouped-completion-gap.cva");
    let result = process(&mut cva, 2, 110);
    let ids = result
        .created
        .iter()
        .map(|memory| memory.id)
        .collect::<Vec<_>>();
    let accepted_turn = cva.rel_turn_count();
    let chunk = completion_chunk(&mut cva);
    let end = chunk.legacy_offset() + 8 + chunk.legacy_len();
    drop(cva);
    fs::OpenOptions::new()
        .write(true)
        .open(&path)
        .unwrap()
        .set_len(end)
        .unwrap();
    let before = fs::read(&path).unwrap();
    let reopened = Cva::open(&path).unwrap();
    assert!(
        reopened
            .freshness_same_publication_cohort(ids[0], ids[1])
            .unwrap()
    );
    assert_eq!(
        reopened.freshness_publication_cohort(ids[0]).unwrap(),
        ids.iter().copied().collect()
    );
    for id in ids {
        let record = reopened.freshness_record(id).unwrap();
        assert_eq!(record.score, 100);
        assert_eq!(record.admitted_at_turn, None);
        assert!(reopened.freshness_is_publication_birth(id));
        assert!(!reopened.freshness_has_pending_publication(id));
        assert_eq!(
            reopened.freshness_record_at(id, accepted_turn).unwrap(),
            Some(record)
        );
    }
    drop(reopened);
    assert_eq!(fs::read(&path).unwrap(), before);
}
use crate::{
    Cva, EpisodeBoundary, EpisodeConfig, EpisodeOrigin, InsomniaExtractor, InsomniaPriority,
    InsomniaWorkState, ObjectRef, SimulatedGeneralEndpoint,
};
use serde_json::{Value, json};
use std::fs;

use super::test_support::test_path;

fn setup(name: &str) -> (std::path::PathBuf, Cva, crate::Episode) {
    let path = test_path(name);
    let mut cva = Cva::create(&path).unwrap();
    let mut parent = None;
    for index in 0..8 {
        let user = format!("u{index}");
        let assistant = format!("a{index}");
        cva.append_node(
            user.clone(),
            "c1".into(),
            parent.clone(),
            "user".into(),
            10 + index as i64 * 2,
            &format!("Atomic Insomnia completion fact {index}."),
        )
        .unwrap();
        cva.append_node(
            assistant.clone(),
            "c1".into(),
            Some(user),
            "assistant".into(),
            11 + index as i64 * 2,
            "Acknowledged.",
        )
        .unwrap();
        parent = Some(assistant);
    }
    let episode = cva
        .materialize_path_episodes(
            "c1",
            parent.as_deref().unwrap(),
            EpisodeConfig::default(),
            EpisodeOrigin::Live,
            Some((EpisodeBoundary::Inactivity, 100)),
        )
        .unwrap()
        .created
        .remove(0);
    cva.queue_insomnia_episode(episode.id, InsomniaPriority::Live, 101)
        .unwrap();
    (path, cva, episode)
}

fn responses(count: usize) -> Vec<Value> {
    let mut turns = serde_json::Map::new();
    for index in 0..8 {
        let clause = if index < count {
            json!({
                "disposition": "retain",
                "authority_kind": "direct",
                "category": "decision",
                "type": "project",
                "lifecycle": "current",
                "proposition": format!("Atomic Insomnia completion invariant {index}."),
                "authority_source_node_id": "",
                "grounding_source_node_id": "",
                "reason": "synthetic durable state"
            })
        } else {
            json!({
                "disposition": "omit",
                "authority_kind": "none",
                "category": "none",
                "type": "none",
                "lifecycle": "none",
                "proposition": "",
                "authority_source_node_id": "",
                "grounding_source_node_id": "",
                "reason": "synthetic omission"
            })
        };
        turns.insert(format!("u{index}"), json!([clause]));
    }
    let mut responses = vec![json!({"turns": turns, "evidence_requests": []})];
    if count > 0 {
        let mut groups = serde_json::Map::new();
        for index in 0..count {
            groups.insert(
                format!("g{index:03}"),
                json!({
                    "title": format!("Atomic completion {index}"),
                    "content": format!("Atomic Insomnia completion invariant {index}.")
                }),
            );
        }
        responses.push(json!({"groups": groups}));
    }
    responses
}

fn process(cva: &mut Cva, count: usize, now: i64) -> crate::InsomniaProcessResult {
    let claim = cva
        .claim_insomnia_episode("worker", now, 1_000)
        .unwrap()
        .unwrap();
    let extractor = InsomniaExtractor::new(SimulatedGeneralEndpoint::new(
        "synthetic-insomnia",
        responses(count),
    ));
    cva.process_claimed_insomnia_episode(&claim, &extractor, "private", now, now + 1)
        .unwrap()
}

fn fail_processing_once(cva: &mut Cva, now: i64) {
    let claim = cva
        .claim_insomnia_episode("worker", now, 1_000)
        .unwrap()
        .unwrap();
    let token = claim.lease_token.unwrap();
    let extractor = InsomniaExtractor::new(SimulatedGeneralEndpoint::new(
        "synthetic-failure",
        Vec::new(),
    ));
    assert!(
        cva.process_claimed_insomnia_episode(&claim, &extractor, "private", now, now + 1)
            .is_err()
    );
    cva.fail_insomnia_episode(
        claim.episode_id,
        token,
        now,
        now + 1,
        now + 2,
        "synthetic processing failure".into(),
    )
    .unwrap();
}

fn completion_chunk(cva: &mut Cva) -> ObjectRef {
    cva.container
        .chunks()
        .unwrap()
        .into_iter()
        .find(|chunk| {
            let payload = cva.container.read(*chunk).unwrap();
            decode_completion(&payload).unwrap().is_some()
        })
        .unwrap()
}

fn completion_chunks(cva: &mut Cva) -> usize {
    cva.container
        .chunks()
        .unwrap()
        .into_iter()
        .filter(|chunk| {
            let payload = cva.container.read(*chunk).unwrap();
            decode_completion(&payload).unwrap().is_some()
        })
        .count()
}

fn legacy_completion_payload(episode_id: crate::EpisodeId, attempt: u32) -> Vec<u8> {
    fn push_string(out: &mut Vec<u8>, value: &str) {
        out.extend_from_slice(&(value.len() as u32).to_le_bytes());
        out.extend_from_slice(value.as_bytes());
    }

    let mut out = Vec::new();
    out.extend_from_slice(b"CVAINSC1");
    out.extend_from_slice(&episode_id.0);
    out.extend_from_slice(&attempt.to_le_bytes());
    out.extend_from_slice(&110_i64.to_le_bytes());
    out.extend_from_slice(&111_i64.to_le_bytes());
    out.extend_from_slice(&0_u32.to_le_bytes());
    push_string(&mut out, "legacy-synthetic");
    push_string(&mut out, "legacy-v1");
    out.extend_from_slice(&0_u32.to_le_bytes());
    out.extend_from_slice(&0_u32.to_le_bytes());
    out
}

#[test]
fn legacy_cvainsc1_completion_reopens_as_final_success() {
    let (path, mut cva, episode) = setup("completion-legacy-v1.cva");
    let claim = cva
        .claim_insomnia_episode("legacy-worker", 110, 1_000)
        .unwrap()
        .unwrap();
    let payload = legacy_completion_payload(episode.id, claim.attempt_count);
    let decoded = decode_completion(&payload).unwrap().unwrap();
    assert!(decoded.bodies.is_empty());
    assert!(decoded.records.is_empty());
    cva.container.append(&payload).unwrap();
    cva.sync().unwrap();
    drop(cva);

    let reopened = Cva::open(&path).unwrap();
    let attempts = reopened.insomnia_attempts(episode.id);
    assert_eq!(attempts.len(), 1);
    assert_eq!(attempts[0].state, InsomniaWorkState::Complete);
    assert_eq!(attempts[0].extractor_model, "legacy-synthetic");
    assert_eq!(
        reopened.insomnia_work(episode.id).unwrap().state,
        InsomniaWorkState::Complete
    );
}

#[test]
fn successful_completion_collapses_retry_history_and_reopens() {
    let (path, mut cva, episode) = setup("completion-retry.cva");
    let first = cva
        .claim_insomnia_episode("worker", 110, 1_000)
        .unwrap()
        .unwrap();
    cva.fail_insomnia_episode(
        episode.id,
        first.lease_token.unwrap(),
        110,
        111,
        120,
        "synthetic retry".into(),
    )
    .unwrap();
    assert!(cva.insomnia_attempts(episode.id).is_empty());
    let result = process(&mut cva, 2, 120);
    assert_eq!(result.created.len(), 2);
    let expected: Vec<_> = result
        .created
        .iter()
        .map(|memory| (memory.id, memory.global_version, memory.memory_version))
        .collect();
    assert_eq!(cva.insomnia_attempts(episode.id).len(), 1);
    assert_eq!(cva.insomnia_attempts(episode.id)[0].attempt, 2);
    cva.sync().unwrap();
    drop(cva);

    let mut reopened = Cva::open(&path).unwrap();
    assert_eq!(reopened.memory_stats().memories, 2);
    for (id, global_version, memory_version) in expected {
        let memory = reopened.memory(id).unwrap();
        assert_eq!(memory.global_version, global_version);
        assert_eq!(memory.memory_version, memory_version);
    }
    let attempts = reopened.insomnia_attempts(episode.id);
    assert_eq!(attempts.len(), 1);
    assert_eq!(attempts[0].attempt, 2);
    assert_eq!(attempts[0].state, InsomniaWorkState::Complete);
    assert_eq!(attempts[0].extractor_model, "synthetic-insomnia");
    assert_eq!(
        reopened.insomnia_work(episode.id).unwrap().state,
        InsomniaWorkState::Complete
    );
}

#[test]
fn zero_and_many_memory_completions_are_single_outcomes() {
    for (name, count) in [("completion-zero.cva", 0), ("completion-many.cva", 8)] {
        let (path, mut cva, episode) = setup(name);
        let chunks_before = cva.container.chunks().unwrap().len();
        let result = process(&mut cva, count, 110);
        assert_eq!(result.created.len(), count);
        assert_eq!(cva.container.chunks().unwrap().len(), chunks_before + 1);
        assert_eq!(completion_chunks(&mut cva), 1);
        drop(cva);
        let reopened = Cva::open(&path).unwrap();
        assert_eq!(reopened.memory_stats().memories, count);
        let attempts = reopened.insomnia_attempts(episode.id);
        assert_eq!(attempts.len(), 1);
        assert_eq!(attempts[0].memory_ids.len(), count);
    }
}

#[test]
fn one_thousand_processing_failures_add_zero_bytes_before_single_append_success() {
    let (path, mut cva, _episode) = setup("completion-thousand-failures.cva");
    cva.sync().unwrap();
    let bytes_before = fs::metadata(&path).unwrap().len();
    let chunks_before = cva.container.chunks().unwrap().len();

    for index in 0..1_000_i64 {
        fail_processing_once(&mut cva, 1_000 + index * 10);
    }
    cva.sync().unwrap();
    assert_eq!(fs::metadata(&path).unwrap().len(), bytes_before);
    assert_eq!(cva.container.chunks().unwrap().len(), chunks_before);
    assert_eq!(completion_chunks(&mut cva), 0);

    let result = process(&mut cva, 3, 20_000);
    assert_eq!(result.created.len(), 3);
    assert_eq!(cva.container.chunks().unwrap().len(), chunks_before + 1);
    assert_eq!(completion_chunks(&mut cva), 1);
}

#[test]
fn truncated_completion_is_recovered_for_zero_one_and_many_memories_then_retries_cleanly() {
    for (case, count) in [("zero", 0), ("one", 1), ("many", 3)] {
        let (path, mut cva, episode) = setup(&format!("completion-truncated-{case}.cva"));
        let original_result = process(&mut cva, count, 110);
        let expected_ids: Vec<_> = original_result
            .created
            .iter()
            .map(|memory| memory.id)
            .collect();
        let commit = completion_chunk(&mut cva);
        assert_eq!(completion_chunks(&mut cva), 1);
        drop(cva);

        let original = fs::read(&path).unwrap();
        let commit_offset = commit.legacy_offset();
        let commit_len = commit.legacy_len();
        let full_end = commit_offset + 8 + commit_len;
        let mut cuts = vec![
            commit_offset + 1,
            commit_offset + 7,
            commit_offset + 8,
            commit_offset + 9,
            commit_offset + 8 + commit_len / 3,
            commit_offset + 8 + commit_len / 2,
            full_end - 1,
        ];
        cuts.sort_unstable();
        cuts.dedup();
        cuts.retain(|cut| *cut > commit_offset && *cut < full_end);

        for (iteration, cut) in cuts.into_iter().enumerate() {
            fs::write(&path, &original[..cut as usize]).unwrap();

            let mut reopened = Cva::open(&path).unwrap();
            assert_eq!(reopened.memory_stats().memories, 0);
            assert_eq!(
                reopened.insomnia_work(episode.id).unwrap().state,
                InsomniaWorkState::Pending
            );
            assert!(reopened.insomnia_attempts(episode.id).is_empty());
            assert_eq!(fs::metadata(&path).unwrap().len(), commit_offset);

            let retry = process(&mut reopened, count, 1_000 + iteration as i64 * 10);
            let retry_ids: Vec<_> = retry.created.iter().map(|memory| memory.id).collect();
            assert_eq!(retry_ids, expected_ids);
            assert_eq!(completion_chunks(&mut reopened), 1);
            drop(reopened);

            let final_open = Cva::open(&path).unwrap();
            assert_eq!(final_open.memory_stats().memories, count);
            assert_eq!(final_open.insomnia_attempts(episode.id).len(), 1);
            assert_eq!(
                final_open.insomnia_work(episode.id).unwrap().state,
                InsomniaWorkState::Complete
            );
            drop(final_open);
        }
    }
}

#[test]
fn v6_completion_carries_explicit_freshness_birth_proof_without_granting_legacy() {
    let (_path, mut cva, _episode) = setup("completion-v6-freshness-proof.cva");
    let result = process(&mut cva, 3, 110);
    assert_eq!(result.created.len(), 3);
    let chunk = completion_chunk(&mut cva);
    let payload = cva.container.read(chunk).unwrap();
    let completion = decode_completion(&payload).unwrap().unwrap();
    assert_eq!(completion.freshness_birth_turn, Some(cva.rel_turn_count()));
    assert_eq!(
        completion.freshness_policy_version,
        Some(crate::freshness::FRESHNESS_POLICY_VERSION as u64)
    );
    assert_eq!(completion.freshness_birth_memory_ids.len(), 3);
    assert!(completion.freshness_birth_memory_ids.iter().all(|id| {
        completion
            .records
            .iter()
            .any(|record| record.id == *id && record.revision == 1)
    }));
}
