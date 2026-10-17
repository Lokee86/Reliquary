use crate::compatibility_profile_codec::encode_profile;
use crate::compatibility_profile_probe::{compatibility_profile_id, profile_from_endpoint};
use crate::{COMPATIBILITY_MIN_COSINE, Cva, SimulatedEmbeddingEndpoint, VectorNormalization};
use std::fs;
use std::path::PathBuf;

fn test_path(name: &str) -> PathBuf {
    let unique = uuid::Uuid::new_v4();
    let dir = std::env::temp_dir().join(format!("continuity-compatibility-{unique}"));
    fs::create_dir_all(&dir).unwrap();
    dir.join(name)
}

fn endpoint(seed: u64) -> SimulatedEmbeddingEndpoint {
    SimulatedEmbeddingEndpoint::new(16, VectorNormalization::L2, seed)
}

#[test]
fn profile_round_trips_and_is_clock_neutral() {
    let path = test_path("roundtrip.cva");
    let mut cva = Cva::create(&path).unwrap();
    cva.append_node(
        "n0".into(),
        "c1".into(),
        None,
        "user".into(),
        0,
        "before profile",
    )
    .unwrap();
    let before_global = cva.record_versions().last().unwrap().global_version;
    let profile = cva.establish_compatibility_profile(&endpoint(1)).unwrap();
    assert_eq!(cva.compatibility_profile_stats().profiles, 1);
    assert_eq!(cva.archive_version(), 1);
    assert_eq!(cva.vector_version(), 0);
    cva.append_node(
        "n1".into(),
        "c1".into(),
        Some("n0".into()),
        "assistant".into(),
        1,
        "after profile",
    )
    .unwrap();
    assert_eq!(
        cva.record_versions().last().unwrap().global_version,
        before_global + 1
    );
    cva.sync().unwrap();
    drop(cva);

    let reopened = Cva::open(path).unwrap();
    assert_eq!(reopened.compatibility_profile(profile.id).unwrap(), profile);
}

#[test]
fn routed_scale_endpoint_drift_is_compatible_and_reuses_profile() {
    let path = test_path("drift.cva");
    let mut cva = Cva::create(path).unwrap();
    let profile = cva.establish_compatibility_profile(&endpoint(2)).unwrap();
    let drifted = endpoint(2).with_drift(0.01);
    let report = cva
        .verify_compatibility_endpoint(profile.id, &drifted)
        .unwrap();
    assert!(report.compatible);
    assert!(report.minimum_cosine.unwrap() >= COMPATIBILITY_MIN_COSINE);
    let reused = cva.establish_compatibility_profile(&drifted).unwrap();
    assert_eq!(reused.id, profile.id);
    assert_eq!(cva.compatibility_profile_stats().profiles, 1);
}

#[test]
fn endpoint_drift_above_the_old_threshold_can_remain_compatible() {
    let path = test_path("larger-drift.cva");
    let mut cva = Cva::create(path).unwrap();
    let profile = cva.establish_compatibility_profile(&endpoint(2)).unwrap();
    let drifted = endpoint(2).with_drift(0.012);
    let report = cva
        .verify_compatibility_endpoint(profile.id, &drifted)
        .unwrap();
    assert!(report.compatible);
    assert!(report.minimum_cosine.unwrap() >= COMPATIBILITY_MIN_COSINE);
}

#[test]
fn stored_v2_profile_keeps_exact_id_and_reopens_under_v3_policy() {
    let path = test_path("v2-reopen.cva");
    let endpoint = endpoint(6);
    let mut cva = Cva::create(&path).unwrap();
    let mut v2 = profile_from_endpoint(&endpoint).unwrap();
    v2.compatibility_policy_version = 2;
    v2.id = compatibility_profile_id(&v2);
    let expected_id = v2.id;
    cva.container.append(&encode_profile(&v2).unwrap()).unwrap();
    cva.sync().unwrap();
    drop(cva);

    let reopened = Cva::open(path).unwrap();
    assert_eq!(reopened.compatibility_profile(expected_id).unwrap(), v2);
    assert_eq!(
        reopened.compatibility_profile(expected_id).unwrap().id,
        expected_id
    );
}

#[test]
fn calibrated_boundary_has_real_drift_near_996_and_v3_floor() {
    let path = test_path("calibrated-boundary.cva");
    let mut cva = Cva::create(path).unwrap();
    let profile = cva.establish_compatibility_profile(&endpoint(7)).unwrap();
    let mut near_996 = None;
    let mut floor_pass = None;
    let mut floor_fail = None;
    let mut near_985 = None;
    for step in 1..=2000 {
        let drift = step as f32 / 10_000.0;
        let report = cva
            .verify_compatibility_endpoint(profile.id, &endpoint(7).with_drift(drift))
            .unwrap();
        let minimum = report.minimum_cosine.unwrap();
        if near_996.is_none() && (minimum - 0.996).abs() < 0.0005 {
            near_996 = Some(minimum);
        }
        if floor_pass.is_none() && minimum >= 0.99 {
            floor_pass = Some(minimum);
        }
        if floor_fail.is_none() && minimum < 0.99 {
            floor_fail = Some(minimum);
        }
        if near_985.is_none() && (minimum - 0.985).abs() < 0.0005 {
            near_985 = Some(minimum);
        }
    }
    assert!(near_996.is_some());
    assert!(floor_pass.is_some_and(|minimum| minimum >= 0.99));
    assert!(floor_fail.is_some_and(|minimum| minimum < 0.99));
    assert!(near_985.is_some());
}

#[test]
fn materially_different_endpoint_gets_a_distinct_profile() {
    let path = test_path("distinct.cva");
    let mut cva = Cva::create(path).unwrap();
    let first = cva.establish_compatibility_profile(&endpoint(3)).unwrap();
    let second_endpoint = endpoint(4);
    let report = cva
        .verify_compatibility_endpoint(first.id, &second_endpoint)
        .unwrap();
    assert!(!report.compatible);
    let second = cva
        .establish_compatibility_profile(&second_endpoint)
        .unwrap();
    assert_ne!(first.id, second.id);
    assert_eq!(cva.compatibility_profile_stats().profiles, 2);
}

#[test]
fn dimensions_are_part_of_compatibility() {
    let path = test_path("dimensions.cva");
    let mut cva = Cva::create(path).unwrap();
    let profile = cva.establish_compatibility_profile(&endpoint(5)).unwrap();
    let other = SimulatedEmbeddingEndpoint::new(8, VectorNormalization::L2, 5);
    let report = cva
        .verify_compatibility_endpoint(profile.id, &other)
        .unwrap();
    assert!(!report.compatible);
    assert_eq!(report.minimum_cosine, None);
}
