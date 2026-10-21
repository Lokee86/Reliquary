use crate::{Cva, FragmentConfig, SimulatedEmbeddingEndpoint, VectorNormalization};
use std::collections::{HashMap, HashSet};

// Independent conversations add immutable fragments without changing prior text.
// Profiles are measured separately so their multiplicity is not called same-profile duplication.
#[test]
fn repeated_same_profile_archive_vector_generation_raw_measurement() {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join(".test-tmp")
        .join(format!("generation-duplication-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&root).unwrap();
    let mut results = Vec::new();
    for dimensions in [12, 384] {
        for profile_count in [1, 2] {
            let path = root.join(format!("{dimensions}-{profile_count}.rel"));
            let mut owner = Cva::create(&path).unwrap();
            let endpoints: Vec<_> = (0..profile_count)
                .map(|i| {
                    SimulatedEmbeddingEndpoint::new(dimensions, VectorNormalization::L2, 500 + i)
                })
                .collect();
            let profiles: Vec<_> = endpoints
                .iter()
                .map(|e| owner.establish_compatibility_profile(e).unwrap())
                .collect();
            let mut previous = HashMap::new();
            let mut unique_rows = HashSet::new();
            let mut matrices = HashSet::new();
            let mut rows = 0;
            let mut unchanged = 0;
            let mut history = Vec::new();
            let mut matrix_bytes = 0;
            let mut manifests = 0;
            for generation in 0..8 {
                let conversation = format!("conversation-{generation}");
                for turn in 0..8 {
                    owner
                        .append_node(
                            format!("{generation}-{turn}"),
                            conversation.clone(),
                            (turn > 0).then(|| format!("{generation}-{}", turn - 1)),
                            if turn % 2 == 0 { "user" } else { "assistant" }.into(),
                            (generation * 8 + turn) as i64,
                            &format!("fixed generation {generation} turn {turn}"),
                        )
                        .unwrap();
                }
                owner
                    .materialize_path_fragments(
                        &conversation,
                        &format!("{generation}-7"),
                        FragmentConfig::default(),
                        false,
                    )
                    .unwrap();
                for (endpoint, profile) in endpoints.iter().zip(&profiles) {
                    let record = owner
                        .build_archive_vector_generation(profile.id, endpoint)
                        .unwrap();
                    let archive = owner.archive_vectors(record.archive_vector_id).unwrap();
                    let packed = owner.packed_vectors(archive.packed_vector_id).unwrap();
                    let exact = packed.bytes().to_vec();
                    let row_bytes = dimensions as usize * 4;
                    assert_eq!(exact.len(), archive.fragment_ids.len() * row_bytes);
                    for (id, bytes) in archive
                        .fragment_ids
                        .iter()
                        .zip(exact.chunks_exact(row_bytes))
                    {
                        rows += 1;
                        if let Some(prior) = previous.insert((profile.id, *id), bytes.to_vec()) {
                            assert_eq!(prior, bytes);
                            unchanged += 1;
                        }
                        unique_rows.insert(bytes.to_vec());
                    }
                    if matrices.insert(archive.packed_vector_id) {
                        matrix_bytes += 72 + exact.len();
                        manifests += 61 + 32 * archive.fragment_ids.len();
                    }
                    history.push((record, archive.packed_vector_id, exact));
                }
            }
            assert!(unchanged > 0);
            owner.sync().unwrap();
            drop(owner);
            let mut reopened = Cva::open(&path).unwrap();
            for (record, id, bytes) in history {
                assert_eq!(
                    reopened.vector_generation_at(
                        record.compatibility_profile_id,
                        record.vector_version
                    ),
                    Some(record)
                );
                assert_eq!(reopened.packed_vectors(id).unwrap().bytes(), bytes);
            }
            let row_bytes = dimensions as usize * 4;
            let candidate_bytes = unique_rows.len() * (53 + row_bytes) + manifests;
            results.push(serde_json::json!({"dimensions":dimensions,"profile_count":profile_count,"generations_per_profile":8,"matrix_count":matrices.len(),"row_occurrences":rows,"unchanged_same_profile_fragment_occurrences":unchanged,"unique_exact_rows_across_profiles":unique_rows.len(),"current_packed_framed_bytes":matrix_bytes,"candidate_row_and_manifest_framed_bytes":candidate_bytes,"historical_exact_bytes_equal":true}));
        }
    }
    let report = serde_json::json!({"schema":1,"date":"2026-10-21","compression_included":false,"workload":"eight independent eight-turn conversations; rebuild every active profile after each materialization","candidate_cost":"53 framed row metadata bytes plus row bytes; 61 manifest bytes plus 32-byte exact row identities; existing Archive-vector mapping unchanged and excluded from both sides","candidate_is_measured_model_not_implemented":true,"scenarios":results});
    println!("{report}");
    if std::env::var_os("RELIQUARY_WRITE_VECTOR_MEASUREMENT").is_some() {
        std::fs::write(
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("benchmarks/archive-vector-generation-duplication-2026-10-21.json"),
            serde_json::to_vec_pretty(&report).unwrap(),
        )
        .unwrap();
    }
    std::fs::remove_dir_all(root).unwrap();
}
