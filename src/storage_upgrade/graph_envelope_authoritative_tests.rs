//! Graph envelope measurements exclude shared reservations and other owners.
use super::*;

fn framed_publications(path: &Path, v1: bool) -> (u64, u64, u64) {
    let mut container = if v1 {
        Container::open_v1_read_only(path).unwrap()
    } else {
        Container::open(path).unwrap()
    };
    let mut count = 0;
    let mut framed = 0;
    let mut compact_link_framed = 0;
    for object in container.chunks().unwrap() {
        let bytes = container.read(object).unwrap();
        if let Some(version) = crate::graph_codec::decode_version(&bytes).unwrap() {
            let carrier = container.read(version.mutation).unwrap();
            let (single, len) =
                if let Some(rows) = crate::graph_compact_codec::decode(&carrier).unwrap() {
                    (carrier.starts_with(b"CVAGCM01"), rows.len())
                } else if let Some(rows) = crate::graph_codec::decode_batch(&carrier).unwrap() {
                    (false, rows.len())
                } else {
                    (
                        true,
                        usize::from(
                            crate::graph_codec::decode_mutation(&carrier)
                                .unwrap()
                                .is_some(),
                        ),
                    )
                };
            assert!(len > 0);
            count += 1;
            framed += (carrier.len() + bytes.len() + 16) as u64;
            compact_link_framed +=
                (if single { 8 } else { 12 }) + len as u64 * 12 + 8 + bytes.len() as u64 + 8;
        } else if let Some(envelope) =
            crate::container::publication_envelope::decode(&bytes).unwrap()
            && crate::graph_compact_codec::publication::decode(
                envelope.body,
                envelope.global_version,
            )
            .unwrap()
            .is_some()
        {
            count += 1;
            framed += bytes.len() as u64 + 8;
        }
    }
    (count, framed, compact_link_framed)
}

#[test]
fn authoritative_v1_graph_envelopes_preserve_six_sources_and_measure_owner_framing() {
    let root = PathBuf::from(
        std::env::var_os("RELIQUARY_HISTORY_FIXTURES").expect("accepted fixture root"),
    );
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let accepted: Value =
        serde_json::from_slice(&fs::read(manifest.join(ACCEPTED_HASHES)).unwrap()).unwrap();
    let scratch = ScratchDirectory::new();
    let mut measurements = Vec::new();
    let mut totals = (0_u64, 0_u64, 0_u64, 0_u64);
    for corpus in CORPORA {
        for (kind, name, file_kind) in [
            ("rel", "project.prj.rel", FileKind::Reliquary),
            ("phy", "user.phy", FileKind::Phylactery),
        ] {
            let key = format!("{corpus}/{name}");
            let source = root.join(&key);
            let original = fs::read(&source).unwrap();
            let hash = sha256(&original);
            assert_eq!(
                hash,
                accepted["authoritative_hashes"][&key].as_str().unwrap()
            );
            let modified = fs::metadata(&source).unwrap().modified().unwrap();
            let before = framed_publications(&source, true);
            let destination = scratch.destination(&format!("envelope-{corpus}-{kind}"));
            let plan = plan_storage_upgrade(StorageUpgradeRequest {
                source: source.clone(),
                destination: destination.clone(),
            })
            .unwrap();
            let staged = super::super::shell::stage_graph_envelope_storage_upgrade(&plan)
                .unwrap_or_else(|error| panic!("stage {key}: {error}"));
            match file_kind {
                FileKind::Reliquary => compare_rel(&source, staged.test_path()),
                FileKind::Phylactery => compare_phy(&source, staged.test_path()),
            }
            let after = framed_publications(staged.test_path(), false);
            assert_eq!(before.0, after.0);
            assert_eq!(after.2, 0, "no linked Graph publications remain");
            assert_source_unchanged(&source, &original, &hash, modified);
            totals.0 += before.0;
            totals.1 += before.1;
            totals.2 += before.2;
            totals.3 += after.1;
            measurements.push(serde_json::json!({"fixture":key,"source_sha256":hash,"source_unchanged":true,"graph_publications":before.0,"legacy_source_graph_framed_bytes":before.1,"compact_carrier_link_framed_bytes":before.2,"native_envelope_framed_bytes":after.1,"compact_to_envelope_saved_bytes":before.2 as i64-after.1 as i64,"mapping_bytes_unchanged":true,"semantic_mismatches":0,"canonical_corpus_promotion":false}));
            drop(staged);
            assert!(!destination.exists());
        }
    }
    let measurement = serde_json::json!({"schema":1,"step":219,"implementation_commit":std::env::var("RELIQUARY_MEASUREMENT_COMMIT").ok(),"graph_publications":totals.0,"legacy_source_graph_framed_bytes":totals.1,"compact_carrier_link_framed_bytes":totals.2,"native_envelope_framed_bytes":totals.3,"compact_to_envelope_saved_bytes":totals.2 as i64-totals.3 as i64,"scope":"accepted Graph carrier/link frames versus native envelopes; shared reservations, mappings and other owners excluded","fixtures":measurements});
    if let Some(output) = std::env::var_os("RELIQUARY_GRAPH_ENVELOPE_MEASUREMENTS") {
        fs::write(output, serde_json::to_vec_pretty(&measurement).unwrap()).unwrap();
    }
}
