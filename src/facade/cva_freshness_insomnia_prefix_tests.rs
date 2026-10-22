use super::*;
use crate::insomnia::completion::{decode_completion, encode_completion};

fn completion_payload(cva: &mut Cva) -> Vec<u8> {
    cva.container
        .chunks()
        .unwrap()
        .into_iter()
        .find_map(|object| {
            let payload = cva.container.read(object).unwrap();
            decode_completion(&payload).unwrap().map(|_| payload)
        })
        .expect("grouped Insomnia completion")
}

#[test]
fn accepted_insomnia_birth_prefix_projects_exact_members_at_local_cut() {
    let (_path, mut cva, _) =
        crate::insomnia::completion_tests::setup("insomnia-birth-prefix-memory-cut.cva");
    let result = crate::insomnia::completion_tests::process(&mut cva, 3, 110);
    let payload = completion_payload(&mut cva);
    let decoded = decode_completion(&payload).unwrap().unwrap();
    let expected_members = decoded
        .records
        .iter()
        .filter(|record| record.revision == 1)
        .map(|record| AcceptedInsomniaBirthMember {
            id: record.id,
            memory_version: record.memory_version,
            global_version: record.global_version,
        })
        .collect::<Vec<_>>();
    assert_eq!(expected_members.len(), result.created.len());

    let empty = cva
        .insomnia_birth_cohort_at_memory_prefix(&payload, 0)
        .unwrap()
        .unwrap();
    assert!(empty.members.is_empty());
    assert_eq!(empty.turn, decoded.freshness_birth_turn.unwrap());
    assert_eq!(
        empty.policy_version,
        crate::freshness::FRESHNESS_POLICY_VERSION as u64
    );

    let partial = cva
        .insomnia_birth_cohort_at_memory_prefix(&payload, 1)
        .unwrap()
        .unwrap();
    assert_eq!(partial.members, expected_members[..1]);

    let full_cut = cva.memory_version();
    let full = cva
        .insomnia_birth_cohort_at_memory_prefix(&payload, full_cut)
        .unwrap()
        .unwrap();
    assert_eq!(full.members, expected_members);
    assert_eq!(full.global_version_start, decoded.global_version_start);

    let mut current_no_proof = decoded.clone();
    current_no_proof.freshness_birth_turn = None;
    current_no_proof.freshness_policy_version = None;
    current_no_proof.freshness_birth_memory_ids.clear();
    let current_no_proof = encode_completion(&current_no_proof).unwrap();
    assert!(
        cva.insomnia_birth_cohort_at_memory_prefix(&current_no_proof, full_cut)
            .unwrap()
            .is_none()
    );
}

#[test]
fn accepted_insomnia_birth_validation_rejects_bad_proof_facts_and_cohort_rows() {
    let (_path, mut cva, _) =
        crate::insomnia::completion_tests::setup("insomnia-birth-prefix-validation.cva");
    crate::insomnia::completion_tests::process(&mut cva, 2, 110);
    let payload = completion_payload(&mut cva);
    let decoded = decode_completion(&payload).unwrap().unwrap();

    let mut bad_policy = decoded.clone();
    bad_policy.freshness_policy_version = Some(0);
    assert!(
        crate::owner_lifecycle::validate_insomnia_birth_cohort(
            &cva.archive,
            &cva.memories,
            &bad_policy
        )
        .is_err()
    );

    let mut bad_turn = decoded.clone();
    bad_turn.freshness_birth_turn = Some(0);
    assert!(
        crate::owner_lifecycle::validate_insomnia_birth_cohort(
            &cva.archive,
            &cva.memories,
            &bad_turn
        )
        .is_err()
    );

    let mut mismatched_member = decoded.clone();
    mismatched_member.freshness_birth_memory_ids[0] = crate::MemoryId([0xFF; 32]);
    assert!(
        crate::owner_lifecycle::validate_insomnia_birth_cohort(
            &cva.archive,
            &cva.memories,
            &mismatched_member
        )
        .is_err()
    );

    let mut mismatched_first_identity = decoded.clone();
    mismatched_first_identity.records[0].mutation_id = "not-the-accepted-mutation".into();
    assert!(
        crate::owner_lifecycle::validate_insomnia_birth_cohort(
            &cva.archive,
            &cva.memories,
            &mismatched_first_identity
        )
        .is_err()
    );

    let mut incomplete_exact_set = decoded;
    incomplete_exact_set.freshness_birth_memory_ids.pop();
    assert!(encode_completion(&incomplete_exact_set).is_err());
}
