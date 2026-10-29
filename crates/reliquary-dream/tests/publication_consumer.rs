use reliquary_dream::{
    DreamEvidenceSide, DreamPairClassification, DreamPairEvidence, DreamRelationDirection,
    DreamRelationKind, DreamVerificationPolicy, PublicationPolicyError, desired_relations,
    validate_classification, verification_gate,
};
use reliquary_memory_core::MemoryId;

#[test]
fn policy_rejects_missing_verification_and_plans_direction() {
    let mut pair = DreamPairClassification {
        model: "test".into(),
        a: MemoryId([1; 32]),
        b: MemoryId([2; 32]),
        relation: DreamRelationKind::Supersedes,
        direction: DreamRelationDirection::BToA,
        evidence: vec![
            DreamPairEvidence {
                side: DreamEvidenceSide::A,
                quote: "a".into(),
            },
            DreamPairEvidence {
                side: DreamEvidenceSide::B,
                quote: "b".into(),
            },
        ],
    };
    assert_eq!(validate_classification(&pair), Ok(()));
    assert_eq!(
        verification_gate(&pair, None, DreamVerificationPolicy::default()),
        Err(PublicationPolicyError::MissingVerification)
    );
    let desired = desired_relations(&pair).unwrap();
    assert_eq!(desired.len(), 1);
    assert!(
        desired
            .iter()
            .any(|(source, target, _)| *source == pair.b && *target == pair.a)
    );
    pair.evidence.pop();
    assert_eq!(
        validate_classification(&pair),
        Err(PublicationPolicyError::InvalidClassification)
    );
}
