//! One latest-converter matrix; source oracles are frozen before reader retirement.
use super::*;
use crate::historical_fingerprint::HistoricalFingerprint;
use crate::{Container, Cva, Phylactery};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
    time::Instant,
};

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|v| format!("{v:02x}")).collect()
}
fn mismatch(a: &Value, b: &Value, path: &str) -> Option<String> {
    if a == b {
        return None;
    }
    match (a, b) {
        (Value::Object(a), Value::Object(b)) => {
            for key in a.keys().chain(b.keys()) {
                if let Some(value) = mismatch(
                    a.get(key).unwrap_or(&Value::Null),
                    b.get(key).unwrap_or(&Value::Null),
                    &format!("{path}/{key}"),
                ) {
                    return Some(value);
                }
            }
        }
        (Value::Array(a), Value::Array(b)) => {
            for index in 0..a.len().max(b.len()) {
                if let Some(value) = mismatch(
                    a.get(index).unwrap_or(&Value::Null),
                    b.get(index).unwrap_or(&Value::Null),
                    &format!("{path}/{index}"),
                ) {
                    return Some(value);
                }
            }
        }
        _ => {}
    }
    Some(format!(
        "{path}: {} != {}",
        a.to_string().chars().take(180).collect::<String>(),
        b.to_string().chars().take(180).collect::<String>()
    ))
}
fn fingerprint(value: HistoricalFingerprint) -> Value {
    json!({"cut":value.local_cut,"event_cut":value.event_cut,"rows":value.rows,"sha256":hex(&value.sha256)})
}
fn series(name: &str, head: u64, mut query: impl FnMut(u64) -> HistoricalFingerprint) -> Value {
    let start = Instant::now();
    let values: Vec<_> = (0..=head).map(|cut| fingerprint(query(cut))).collect();
    println!(
        "exhaustive {name}: {} cuts in {}ms",
        head + 1,
        start.elapsed().as_millis()
    );
    json!({"exhaustive":true,"head":head,"fingerprints":values})
}

fn common(
    container: &mut Container,
    memories: &crate::memory_store::MemoryStore,
    entities: &crate::entity_store::EntityStore,
    relationships: &crate::relationship_store::RelationshipStore,
    graph: &crate::graph_store::GraphStore,
    ego: &crate::ego_store::EgoStore,
) -> Value {
    let memory =
        crate::historical_memory_fingerprint::MemoryHistory::read(container, memories).unwrap();
    let memory = series("Memory", memories.memory_version(), |cut| {
        memory.at(cut).unwrap().fingerprint
    });
    let entity = series("Entity", entities.entity_version(), |cut| {
        crate::historical_entity_fingerprint::entity_cut(container, entities, cut)
            .unwrap()
            .fingerprint
    });
    let relationship = series(
        "Relationship",
        relationships.relationship_version(),
        |cut| {
            crate::historical_relationship_fingerprint::relationship_cut(
                container,
                relationships,
                cut,
            )
            .unwrap()
            .fingerprint
        },
    );
    let history =
        crate::historical_graph_fingerprint::GraphHistory::read(container, graph).unwrap();
    let start = Instant::now();
    let full = history.all_cut_fingerprints(false).unwrap();
    for value in [full.first().unwrap(), full.last().unwrap()] {
        assert_eq!(
            *value,
            history.at(value.local_cut, false).unwrap().fingerprint
        );
    }
    let graph_all = json!({"exhaustive":true,"head":graph.graph_version(),"fingerprints":full.into_iter().map(fingerprint).collect::<Vec<_>>()});
    let memory_only = history.all_cut_fingerprints(true).unwrap();
    for value in [memory_only.first().unwrap(), memory_only.last().unwrap()] {
        assert_eq!(
            *value,
            history.at(value.local_cut, true).unwrap().fingerprint
        );
    }
    let memory_graph: Vec<_> = memory_only.into_iter().map(fingerprint).collect();
    println!(
        "exhaustive cached Graph/MemoryGraph in {}ms",
        start.elapsed().as_millis()
    );
    let history = crate::historical_ego_fingerprint::EgoHistory::read(container, ego).unwrap();
    let ego = series("Ego", ego.version(), |cut| {
        history.at(cut).unwrap().fingerprint
    });
    json!({"memory":memory,"entity":entity,"relationship":relationship,"graph":graph_all,"memory_graph":memory_graph,"ego":ego})
}
fn ordering(
    head: u64,
    mut query: impl FnMut(u64) -> (crate::historical_global_cuts::GlobalOwnerCuts, Option<i64>),
) -> Value {
    let mut hash = Sha256::new();
    for global in 0..=head {
        let (c, time) = query(global);
        hash.update(
            serde_json::to_vec(&json!([
                global,
                time,
                c.archive,
                c.memory,
                c.entity,
                c.relationship,
                c.graph,
                c.memory_graph,
                c.vector_generation
            ]))
            .unwrap(),
        );
    }
    json!({"exhaustive":true,"points":head+1,"sha256":hex(&hash.finalize())})
}
fn whole(owner: &mut Cva, cut: u64) -> Value {
    match crate::historical_whole_owner_fingerprint::whole_owner_at(owner,cut){Ok(v)=>fingerprint(v.fingerprint),Err(crate::historical_whole_owner_fingerprint::GlobalProjectionError::MissingArchiveReference)=>json!({"cut":cut,"rejected":"missing Archive reference"}),Err(error)=>panic!("whole-owner cut {cut}: {error}")}
}
fn global_points(owner: &mut Cva) -> Vec<u64> {
    let head = owner.latest_global_version();
    let start = Instant::now();
    let _ = whole(owner, head);
    let probe = start.elapsed().as_secs_f64();
    let estimate = probe * (head + 1) as f64;
    if estimate <= 600.0 {
        println!("global projection exhaustive: probe={probe:.3}s estimate={estimate:.3}s");
        return (0..=head).collect();
    }
    let points = BTreeSet::from([0, head.min(1), head / 4, head / 2, (head / 4) * 3, head]);
    println!(
        "global projection deterministic sampling: head probe={probe:.3}s; exhaustive estimated {estimate:.3}s exceeds600s; {} quartile/end/oldest points; coordinate/time ordering remains exhaustive",
        points.len()
    );
    points.into_iter().collect()
}
fn archive_points(owner: &mut Cva) -> Vec<u64> {
    let head = owner.archive_version();
    let start = Instant::now();
    crate::historical_archive_fingerprint::archive_cut(owner, head).unwrap();
    let probe = start.elapsed().as_secs_f64();
    let estimated = probe * (head + 1) as f64 / 2.0;
    if estimated <= 600.0 {
        println!("Archive snapshots exhaustive: head probe {probe:.3}s estimate {estimated:.3}s");
        (0..=head).collect()
    } else {
        println!(
            "Archive snapshots sampled: head probe {probe:.3}s estimated {estimated:.3}s exceeds600s; all publications/dependencies/prefixes remain exhaustive"
        );
        BTreeSet::from([0, head.min(1), head / 4, head / 2, (head / 4) * 3, head])
            .into_iter()
            .collect()
    }
}
fn archive_prefixes(owner: &mut Cva) -> Vec<String> {
    use crate::archive_codec::publication::{self, ArchivePublication};
    let mut hash = Sha256::new();
    hash.update(b"reliquary:archive-publication-prefix:v1\0");
    hash.update(owner.owner_uuid().unwrap());
    let mut values = vec![hex(&hash.clone().finalize())];
    for local in 1..=owner.archive_version() {
        let version = *owner.archive.record_version(local).unwrap();
        let (record, origin) = owner
            .archive
            .read_publication(&mut owner.container, version)
            .unwrap();
        let value = if origin.archive_version == local {
            ArchivePublication::Record {
                archive_version: local,
                record,
            }
        } else {
            ArchivePublication::Repeat {
                archive_version: local,
                first_applying_archive_version: origin.archive_version,
            }
        };
        let bytes = publication::encode(&value).unwrap();
        hash.update(local.to_le_bytes());
        hash.update(version.global_version.to_le_bytes());
        match owner.transaction_time_ns(version.global_version) {
            Some(time) => {
                hash.update([1]);
                hash.update(time.to_le_bytes());
            }
            None => hash.update([0]),
        };
        hash.update((bytes.len() as u64).to_le_bytes());
        hash.update(bytes);
        values.push(hex(&hash.clone().finalize()));
    }
    values
}
fn rel(owner: &mut Cva, points: &[u64], archive_queries: &[u64]) -> Value {
    let oracle_cuts = BTreeSet::from([0, owner.rel_turn_count() / 2, owner.rel_turn_count()]);
    let original_freshness = owner.freshness.clone();
    let reference: Vec<_> = oracle_cuts
        .iter()
        .map(|cut| {
            crate::historical_freshness_fingerprint::freshness_at_turn(owner, *cut)
                .unwrap()
                .fingerprint
        })
        .collect();
    owner.freshness = owner.freshness.with_cached_oracle_journal().unwrap();
    for (cut, expected) in oracle_cuts.iter().zip(reference) {
        assert_eq!(
            crate::historical_freshness_fingerprint::freshness_at_turn(owner, *cut)
                .unwrap()
                .fingerprint,
            expected
        );
    }
    let base = common(
        &mut owner.container,
        &owner.memories,
        &owner.entities,
        &owner.relationships,
        &owner.graph,
        &owner.ego,
    );
    let archive = json!({"snapshot_queries_exhaustive":archive_queries.len()as u64==owner.archive_version()+1,"head":owner.archive_version(),"snapshots":archive_queries.iter().map(|cut|fingerprint(crate::historical_archive_fingerprint::archive_cut(owner,*cut).unwrap().fingerprint)).collect::<Vec<_>>(),"publication_prefixes_exhaustive":archive_prefixes(owner)});
    let freshness = series("Freshness activity", owner.rel_turn_count(), |cut| {
        crate::historical_freshness_fingerprint::freshness_at_turn(owner, cut)
            .unwrap()
            .fingerprint
    });
    let mut events = BTreeSet::new();
    for entry in owner.freshness.retained_history_entries().unwrap() {
        if let crate::freshness_storage::FreshnessReplayEntry::Event {
            event_id,
            source_turn,
            ..
        } = entry
        {
            events.insert((source_turn, event_id));
        }
    }
    let event_cuts:Vec<_>=events.into_iter().map(|(turn,id)|{let cut=crate::FreshnessEventCut{rel_turn:turn,event_id_inclusive:Some(id.clone()),graph_version:owner.memory_graph_version()};let result=crate::historical_freshness_fingerprint::freshness_at_event(owner,&cut);json!({"turn":turn,"event":id,"outcome":match result{Ok(v)=>fingerprint(v.fingerprint),Err(e)=>json!({"rejected":e})}})}).collect();
    let vectors =
        crate::historical_vector_generation_fingerprint::VectorHistory::read(owner).unwrap();
    let vectors = series("vector generation", owner.vector_version(), |cut| {
        vectors.at(cut).unwrap().fingerprint
    });
    let pairs = fingerprint(
        crate::historical_dream_pair_fingerprint::reliquary_dream_pairs(owner)
            .unwrap()
            .fingerprint,
    );
    let processing = fingerprint(
        crate::historical_dream_processing_fingerprint::reliquary_dream_processing(owner)
            .unwrap()
            .fingerprint,
    );
    let dream =
        fingerprint(crate::historical_dream_proof_fingerprint::dream_proof_state(owner).unwrap());
    let positions =
        crate::historical_graph_fingerprint::GraphHistory::read(&owner.container, &owner.graph)
            .unwrap()
            .memory_graph_positions();
    for global in [
        0,
        owner.latest_global_version() / 2,
        owner.latest_global_version(),
    ] {
        assert_eq!(
            crate::historical_global_cuts::reliquary_global_cuts(owner, global).unwrap(),
            crate::historical_global_cuts::reliquary_global_cuts_with_positions(
                owner, global, &positions
            )
            .unwrap()
        );
    }
    let ordering = ordering(owner.latest_global_version(), |g| {
        (
            crate::historical_global_cuts::reliquary_global_cuts_with_positions(
                owner, g, &positions,
            )
            .unwrap(),
            owner.transaction_time_ns(g),
        )
    });
    let global: Vec<_> = points.iter().map(|g| whole(owner, *g)).collect();
    let result = json!({"common":base,"archive":archive,"freshness":freshness,"freshness_events":event_cuts,"vectors":vectors,"dream_pairs":pairs,"dream_processing":processing,"dream_proof":dream,"global_ordering":ordering,"global_projection":global});
    owner.freshness = original_freshness;
    result
}
fn phy(owner: &mut Phylactery) -> Value {
    let base = common(
        &mut owner.container,
        &owner.memories,
        &owner.entities,
        &owner.relationships,
        &owner.graph,
        &owner.ego,
    );
    let pairs = fingerprint(
        crate::historical_dream_pair_fingerprint::phylactery_dream_pairs(owner)
            .unwrap()
            .fingerprint,
    );
    let processing = fingerprint(
        crate::historical_dream_processing_fingerprint::phylactery_dream_processing(owner)
            .unwrap()
            .fingerprint,
    );
    let positions =
        crate::historical_graph_fingerprint::GraphHistory::read(&owner.container, &owner.graph)
            .unwrap()
            .memory_graph_positions();
    for global in [
        0,
        owner.latest_global_version() / 2,
        owner.latest_global_version(),
    ] {
        assert_eq!(
            crate::historical_global_cuts::phylactery_global_cuts(owner, global).unwrap(),
            crate::historical_global_cuts::phylactery_global_cuts_with_positions(
                owner, global, &positions
            )
            .unwrap()
        );
    }
    let order = ordering(owner.latest_global_version(), |g| {
        (
            crate::historical_global_cuts::phylactery_global_cuts_with_positions(
                owner, g, &positions,
            )
            .unwrap(),
            owner.transaction_time_ns(g),
        )
    });
    json!({"common":base,"dream_pairs":pairs,"dream_processing":processing,"global_ordering":order})
}
fn frames(path: &Path, legacy: bool) -> Value {
    let mut owner = if legacy {
        Container::open_v1_read_only(path).unwrap()
    } else {
        Container::open(path).unwrap()
    };
    let mut families = BTreeMap::<String, (u64, u64)>::new();
    owner
        .visit_payloads::<crate::ContainerError>(|_, bytes| {
            let tag = if bytes.iter().all(|v| *v == 0) {
                "zero physical extent".into()
            } else {
                String::from_utf8_lossy(&bytes[..bytes.len().min(8)]).into_owned()
            };
            let item = families.entry(tag).or_default();
            item.0 += 1;
            item.1 += bytes.len() as u64 + 8;
            Ok(())
        })
        .unwrap();
    let total = fs::metadata(path).unwrap().len();
    assert_eq!(families.values().map(|v| v.1).sum::<u64>() + 40, total);
    json!({"total_raw_file_bytes":total,"header_bytes":40,"families":families})
}

fn retained_evidence(path: &Path, legacy: bool) -> Vec<Vec<u8>> {
    let mut scan = if legacy {
        Container::open_v1_read_only(path).unwrap()
    } else {
        Container::open(path).unwrap()
    };
    let mut values = Vec::new();
    scan.visit_payloads::<crate::MemoryError>(|_, bytes| {
        if legacy {
            if crate::entity_resolution_codec::retained::is_source(bytes) {
                values.push(bytes.to_vec());
            }
        } else {
            assert!(!crate::entity_resolution_codec::retained::is_source(bytes));
            if let Some(raw) = crate::entity_resolution_codec::retained::decode(bytes)? {
                values.push(raw.to_vec());
            }
        }
        Ok(())
    })
    .unwrap();
    values
}

#[test]
fn canonical_six_source_conversion_and_owner_admission() {
    let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let baseline: Value = serde_json::from_slice(
        &fs::read(repo.join("benchmarks/authoritative-history-rewrite-step62-2026-10-18.json"))
            .unwrap(),
    )
    .unwrap();
    let source_root = PathBuf::from(baseline["fixture_root"].as_str().unwrap());
    let directory = repo
        .join(".test-tmp")
        .join(format!("canonical-converted-{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(&directory).unwrap();
    let mut rows = Vec::new();
    for (key, digest) in baseline["authoritative_hashes"].as_object().unwrap() {
        println!("standalone canonical conversion start {key}");
        let path = source_root.join(key);
        let original = fs::read(&path).unwrap();
        assert_eq!(hex(&Sha256::digest(&original)), digest.as_str().unwrap());
        let modified = fs::metadata(&path).unwrap().modified().unwrap();
        let destination = directory.join(key);
        fs::create_dir_all(destination.parent().unwrap()).unwrap();
        let plan = plan_storage_upgrade(StorageUpgradeRequest {
            source: path.clone(),
            destination: destination.clone(),
        })
        .unwrap();
        let start = Instant::now();
        let stage = stage_graph_envelope_storage_upgrade(&plan).unwrap();
        let migration_ms = start.elapsed().as_millis();
        assert_eq!(
            retained_evidence(&path, true),
            retained_evidence(stage.test_path(), false)
        );
        let heads = if key.ends_with(".rel") {
            let old = Cva::open_v1_read_only(&path).unwrap();
            let mut current = Cva::open(stage.test_path()).unwrap();
            current.validate_historical_import_settlements().unwrap();
            current.validate_import_completion_settlements().unwrap();
            assert_eq!(old.owner_uuid(), current.owner_uuid());
            let old_heads = (
                old.latest_global_version(),
                old.archive_version(),
                old.memory_version(),
                old.entities.entity_version(),
                old.relationships.relationship_version(),
                old.graph_version(),
                old.memory_graph_version(),
                old.vector_version(),
                old.rel_turn_count(),
            );
            let current_heads = (
                current.latest_global_version(),
                current.archive_version(),
                current.memory_version(),
                current.entities.entity_version(),
                current.relationships.relationship_version(),
                current.graph_version(),
                current.memory_graph_version(),
                current.vector_version(),
                current.rel_turn_count(),
            );
            assert_eq!(old_heads, current_heads);
            json!(old_heads)
        } else {
            let old = Phylactery::open_v1_read_only(&path).unwrap();
            let current = Phylactery::open(stage.test_path()).unwrap();
            assert_eq!(old.owner_uuid(), current.owner_uuid());
            let old_heads = (
                old.latest_global_version(),
                old.memory_version(),
                old.entities.entity_version(),
                old.relationships.relationship_version(),
                old.graph_version(),
                old.memory_graph_version(),
            );
            let current_heads = (
                current.latest_global_version(),
                current.memory_version(),
                current.entities.entity_version(),
                current.relationships.relationship_version(),
                current.graph_version(),
                current.memory_graph_version(),
            );
            assert_eq!(old_heads, current_heads);
            json!(old_heads)
        };
        let source_frames = frames(&path, true);
        let canonical_frames = frames(stage.test_path(), false);
        for retired in [
            "CVAFRS03", "CVAFRS04", "CVAFRS05", "CVAILEG1", "CVAILEG2", "CVAERS02",
        ] {
            assert!(canonical_frames["families"].get(retired).is_none());
        }
        let canonical = fs::read(stage.test_path()).unwrap();
        fs::write(&destination, &canonical).unwrap();
        assert_eq!(fs::read(&path).unwrap(), original);
        assert_eq!(fs::metadata(&path).unwrap().modified().unwrap(), modified);
        rows.push(json!({"fixture":key,"source_sha256":digest,"canonical_sha256":hex(&Sha256::digest(&canonical)),"canonical_copy":destination,"source_unchanged":true,"owner_admission":true,"heads":heads,"migration_ms":migration_ms,"source_raw":source_frames,"canonical_raw":canonical_frames}));
        let report = json!({"schema":1,"date":"2026-10-21","historical_equivalence_complete":false,"authoritative_promotion":false,"compression_included":false,"fixtures":rows});
        fs::write(
            repo.join("benchmarks/canonical-history-conversion-2026-10-21.json"),
            serde_json::to_vec_pretty(&report).unwrap(),
        )
        .unwrap();
        println!("standalone canonical conversion admitted {key} in {migration_ms}ms");
    }
    assert_eq!(rows.len(), 6);
}

#[test]
fn canonical_six_source_migration_and_historical_equivalence() {
    let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let baseline: Value = serde_json::from_slice(
        &fs::read(repo.join("benchmarks/authoritative-history-rewrite-step62-2026-10-18.json"))
            .unwrap(),
    )
    .unwrap();
    let source_root = PathBuf::from(baseline["fixture_root"].as_str().unwrap());
    let run = std::env::var("RELIQUARY_CANONICAL_MATRIX_RUN")
        .unwrap_or_else(|_| format!("canonical-verified-{}", uuid::Uuid::new_v4()));
    assert!(
        run.starts_with("canonical-verified-")
            && run.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
    );
    let directory = repo.join(".test-tmp").join(run);
    fs::create_dir_all(&directory).unwrap();
    println!("canonical matrix directory {}", directory.display());
    let mut rows: Vec<Value> = if directory.join("progress.json").exists() {
        let progress: Value =
            serde_json::from_slice(&fs::read(directory.join("progress.json")).unwrap()).unwrap();
        assert_eq!(progress["schema"], 2);
        progress["fixtures"].as_array().unwrap().clone()
    } else {
        Vec::new()
    };
    for (key, digest) in baseline["authoritative_hashes"].as_object().unwrap() {
        println!("canonical migration start {key}");
        let path = source_root.join(key);
        let original = fs::read(&path).unwrap();
        assert_eq!(hex(&Sha256::digest(&original)), digest.as_str().unwrap());
        if let Some(row) = rows.iter().find(|row| row["fixture"] == key.as_str()) {
            assert_eq!(row["source_sha256"], *digest);
            let output = directory.join("canonical-v2").join(key);
            let frozen = directory.join("frozen-v1").join(key);
            assert_eq!(
                hex(&Sha256::digest(fs::read(output).unwrap())),
                row["canonical_sha256"].as_str().unwrap()
            );
            assert_eq!(
                hex(&Sha256::digest(fs::read(frozen).unwrap())),
                digest.as_str().unwrap()
            );
            println!("resumed verified {key}");
            continue;
        }
        let modified = fs::metadata(&path).unwrap().modified().unwrap();
        let legacy = directory.join("frozen-v1").join(key);
        fs::create_dir_all(legacy.parent().unwrap()).unwrap();
        fs::write(&legacy, &original).unwrap();
        let destination = directory.join("canonical-v2").join(key);
        fs::create_dir_all(destination.parent().unwrap()).unwrap();
        let plan = plan_storage_upgrade(StorageUpgradeRequest {
            source: path.clone(),
            destination: destination.clone(),
        })
        .unwrap();
        let start = Instant::now();
        let stage = stage_graph_envelope_storage_upgrade(&plan).unwrap();
        let migration_ms = start.elapsed().as_millis();
        assert_eq!(
            retained_evidence(&path, true),
            retained_evidence(stage.test_path(), false),
            "literal retained Entity Resolution evidence for {key}"
        );
        let (expected, actual, points) = if key.ends_with(".rel") {
            let mut old = Cva::open_v1_read_only(&path).unwrap();
            let mut new = Cva::open(stage.test_path()).unwrap();
            super::archive_envelope_authoritative_tests::compare_archive(&mut old, &mut new);
            let points = global_points(&mut old);
            let archives = archive_points(&mut old);
            let expected = rel(&mut old, &points, &archives);
            let actual = rel(&mut new, &points, &archives);
            (expected, actual, points)
        } else {
            let mut old = Phylactery::open_v1_read_only(&path).unwrap();
            let mut new = Phylactery::open(stage.test_path()).unwrap();
            (phy(&mut old), phy(&mut new), Vec::new())
        };
        if let Some(difference) = mismatch(&expected, &actual, "") {
            panic!("owner-history mismatch for {key} at {difference}");
        }
        let old_frames = frames(&path, true);
        let new_frames = frames(stage.test_path(), false);
        let canonical = fs::read(stage.test_path()).unwrap();
        let hash = hex(&Sha256::digest(&canonical));
        fs::write(&destination, &canonical).unwrap();
        assert_eq!(fs::read(&path).unwrap(), original);
        assert_eq!(fs::metadata(&path).unwrap().modified().unwrap(), modified);
        rows.push(json!({"fixture":key,"source_sha256":digest,"canonical_sha256":hash,"source_unchanged":true,"frozen_v1_copy":legacy,"verified_canonical_copy":destination,"migration_ms":migration_ms,"source_raw":old_frames,"canonical_raw":new_frames,"global_projection_points":points,"oracle":expected}));
        let report = json!({"schema":2,"date":"2026-10-21","compression_included":false,"authoritative_sources_unchanged":true,"all_owner_domain_mismatches":0,"fixtures":rows});
        fs::write(
            directory.join("progress.json"),
            serde_json::to_vec_pretty(&report).unwrap(),
        )
        .unwrap();
        println!("canonical migration verified {key}");
    }
    fs::copy(
        directory.join("progress.json"),
        repo.join("benchmarks/canonical-history-equivalence-2026-10-21.json"),
    )
    .unwrap();
}
