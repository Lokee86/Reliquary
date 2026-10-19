use super::*;
use crate::{Cva, FragmentConfig, SimulatedEmbeddingEndpoint, VectorNormalization};
use std::{collections::BTreeMap, fs, path::PathBuf};

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join(".test-tmp")
            .join(format!("vector-generation-oracle-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }

    fn path(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn endpoint(seed: u64) -> SimulatedEmbeddingEndpoint {
    SimulatedEmbeddingEndpoint::new(12, VectorNormalization::L2, seed)
}

fn append_turns(cva: &mut Cva, start: usize, end: usize) {
    for index in start..end {
        cva.append_node(
            format!("n{index}"),
            "c1".into(),
            (index > 0).then(|| format!("n{}", index - 1)),
            if index % 2 == 0 { "user" } else { "assistant" }.into(),
            index as i64,
            &format!("turn {index}"),
        )
        .unwrap();
    }
}

fn materialize(cva: &mut Cva, leaf: usize) {
    cva.materialize_path_fragments("c1", &format!("n{leaf}"), FragmentConfig::default(), false)
        .unwrap();
}

fn assert_public_activation(cva: &Cva, cut: u64, actual: &VectorCut) {
    let expected: BTreeMap<_, _> = cva
        .compatibility_profiles()
        .into_iter()
        .filter_map(|profile| {
            cva.vector_generation_at(profile.id, cut)
                .map(|generation| (profile.id.0, generation))
        })
        .collect();
    assert_eq!(actual.current, expected, "activation mismatch at cut {cut}");
}

#[test]
fn vector_generation_cuts_match_activation_and_survive_append_reopen_and_retirement() {
    let fixture = Fixture::new();
    let path = fixture.path("owner.rel");
    let mut cva = Cva::create(&path).unwrap();
    append_turns(&mut cva, 0, 8);
    materialize(&mut cva, 7);
    let profile = cva.establish_compatibility_profile(&endpoint(41)).unwrap();
    let first = cva
        .build_archive_vector_generation(profile.id, &endpoint(41))
        .unwrap();

    let before_tail = VectorHistory::read(&mut cva).unwrap();
    let cut0 = before_tail.at(0).unwrap();
    let cut1 = before_tail.at(1).unwrap();
    assert!(cut0.current.is_empty());
    assert_eq!(cut1.current.get(&profile.id.0), Some(&first));
    assert_eq!(before_tail.at(1).unwrap().fingerprint, cut1.fingerprint);
    assert!(before_tail.at(2).is_err());
    assert_public_activation(&cva, 0, &cut0);
    assert_public_activation(&cva, 1, &cut1);

    append_turns(&mut cva, 8, 14);
    materialize(&mut cva, 13);
    let second = cva
        .build_archive_vector_generation(profile.id, &endpoint(41))
        .unwrap();
    assert!(second.vector_version > first.vector_version);
    let history = VectorHistory::read(&mut cva).unwrap();
    let later_cut0 = history.at(0).unwrap();
    let later_cut1 = history.at(1).unwrap();
    let cut2 = history.at(2).unwrap();
    assert_eq!(cut0.fingerprint, later_cut0.fingerprint);
    assert_eq!(cut1.fingerprint, later_cut1.fingerprint);
    assert_ne!(cut1.fingerprint.sha256, cut2.fingerprint.sha256);
    assert_eq!(cut2.current.get(&profile.id.0), Some(&second));
    for cut in [0, 1, 2] {
        assert_public_activation(&cva, cut, &history.at(cut).unwrap());
    }

    // Retirement affects present live eligibility, not retained activation history.
    cva.retire_compatibility_profile(profile.id).unwrap();
    assert!(cva.current_vector_generation(profile.id).is_none());
    assert_eq!(cva.historical_vector_generation(profile.id), Some(second));
    assert_eq!(
        history.at(2).unwrap().current.get(&profile.id.0),
        Some(&second)
    );

    cva.sync().unwrap();
    drop(cva);
    let mut reopened = Cva::open(&path).unwrap();
    let reopened_history = VectorHistory::read(&mut reopened).unwrap();
    for cut in [0, 1, 2] {
        let projected = reopened_history.at(cut).unwrap();
        assert_eq!(projected.fingerprint, history.at(cut).unwrap().fingerprint);
        assert_public_activation(&reopened, cut, &projected);
    }
    assert!(reopened.current_vector_generation(profile.id).is_none());
    assert_eq!(
        reopened.historical_vector_generation(profile.id),
        Some(second)
    );
}

#[test]
fn pruned_generation_keeps_activation_and_fingerprints_explicit_payload_unavailability() {
    let fixture = Fixture::new();
    let source = fixture.path("source.rel");
    let output = fixture.path("pruned.rel");
    let mut cva = Cva::create(&source).unwrap();
    append_turns(&mut cva, 0, 6);
    cva.materialize_path_fragments("c1", "n5", FragmentConfig::default(), true)
        .unwrap();
    let endpoint = SimulatedEmbeddingEndpoint::new(512, VectorNormalization::L2, 73);
    let profile = cva.establish_compatibility_profile(&endpoint).unwrap();
    let generation = cva
        .build_archive_vector_generation(profile.id, &endpoint)
        .unwrap();
    let packed_id = cva
        .archive_vectors(generation.archive_vector_id)
        .unwrap()
        .packed_vector_id;
    let cut = generation.vector_version;
    let before_prune = VectorHistory::read(&mut cva).unwrap().at(cut).unwrap();

    cva.retire_compatibility_profile(profile.id).unwrap();
    cva.sync().unwrap();
    cva.prune_tombstoned_history(
        &output,
        crate::HistoryPruneRequest {
            targets: vec![
                crate::owner_lifecycle::LifecycleTarget::CompatibilityProfile(profile.id),
            ],
        },
    )
    .unwrap();

    let mut pruned = Cva::open(&output).unwrap();
    assert_eq!(
        pruned.historical_vector_generation(profile.id),
        Some(generation)
    );
    assert!(matches!(
        pruned.packed_vectors(packed_id),
        Err(crate::PackedVectorError::PayloadUnavailable(
            crate::owner_lifecycle::PayloadUnavailable::Pruned
        ))
    ));
    let first_history = VectorHistory::read(&mut pruned).unwrap();
    let first_cut = first_history.at(cut).unwrap();
    assert_eq!(first_cut.current.get(&profile.id.0), Some(&generation));
    assert_ne!(
        before_prune.fingerprint.sha256,
        first_cut.fingerprint.sha256
    );
    let matrix = &first_history.matrices[&packed_id.0];
    assert!(matrix["payload_hex"].is_null());
    let digest = matrix["prune_manifest_digest"].as_array().unwrap();
    assert_eq!(digest.len(), 32);
    assert!(digest.iter().any(|byte| byte.as_u64().unwrap() != 0));
    assert_eq!(
        first_history.at(cut).unwrap().fingerprint,
        first_cut.fingerprint
    );
    drop(pruned);

    let mut reopened = Cva::open(&output).unwrap();
    let reopened_history = VectorHistory::read(&mut reopened).unwrap();
    let reopened_cut = reopened_history.at(cut).unwrap();
    assert_eq!(reopened_cut.fingerprint, first_cut.fingerprint);
    assert_eq!(&reopened_history.matrices[&packed_id.0], matrix);
    assert_eq!(
        reopened.historical_vector_generation(profile.id),
        Some(generation)
    );
    assert!(matches!(
        reopened.packed_vectors(packed_id),
        Err(crate::PackedVectorError::PayloadUnavailable(
            crate::owner_lifecycle::PayloadUnavailable::Pruned
        ))
    ));
}

#[test]
#[cfg(feature = "history-oracle-fixtures")]
fn authoritative_vector_generation_history_fingerprints_deterministically() {
    let root = PathBuf::from(std::env::var_os("RELIQUARY_HISTORY_FIXTURES").expect("fixture root"));
    let fixture = Fixture::new();
    for (index, corpus) in [
        "chatgpt-first14d",
        "chatgpt-first28d",
        "rhelm-david-r-ellis-pre-relationship",
    ]
    .iter()
    .enumerate()
    {
        let path = fixture.path(&format!("{index}.rel"));
        let mut source = fs::File::open(root.join(corpus).join("project.prj.rel")).unwrap();
        let mut destination = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .unwrap();
        std::io::copy(&mut source, &mut destination).unwrap();
        drop(destination);

        let mut cva = Cva::open(&path).unwrap();
        let first_history = VectorHistory::read(&mut cva).unwrap();
        let second_history = VectorHistory::read(&mut cva).unwrap();
        let head = cva.vector_version();
        let mut cuts = vec![0, head / 2, head];
        cuts.sort_unstable();
        cuts.dedup();
        for cut in cuts {
            eprintln!("Vector Generation fixture {corpus}: fingerprint cut {cut}/{head}");
            let first = first_history.at(cut).unwrap();
            let second = second_history.at(cut).unwrap();
            assert_eq!(first.fingerprint, second.fingerprint);
            assert_eq!(first.current, second.current);
            assert_public_activation(&cva, cut, &first);
        }
    }
}
