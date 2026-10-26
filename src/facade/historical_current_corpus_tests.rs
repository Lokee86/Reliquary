//! Current-format corpus acceptance against the frozen pre-retirement oracle.
use crate::historical_fingerprint::HistoricalFingerprint;
use crate::{Cva, Phylactery};
use reliquary_container::Container;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{collections::BTreeSet, fs, path::PathBuf, time::Instant};

pub(crate) fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|v| format!("{v:02x}")).collect()
}
pub(crate) fn mismatch(a: &Value, b: &Value, path: &str) -> Option<String> {
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
    external: &crate::external_source::publication::ExternalPublicationState,
    entities: &crate::entity_store::EntityStore,
    relationships: &reliquary_perception_store::relationship_store::RelationshipStore,
    graph: &crate::graph_owner::GraphOwner,
    ego: &crate::ego_store::EgoStore,
) -> Value {
    let memory =
        crate::historical_memory_fingerprint::MemoryHistory::read(container, memories, external)
            .unwrap();
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
pub(crate) fn global_points(owner: &mut Cva) -> Vec<u64> {
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
pub(crate) fn archive_points(owner: &mut Cva) -> Vec<u64> {
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
    use reliquary_archive::archive_codec::publication::{self, ArchivePublication};
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
pub(crate) fn rel(owner: &mut Cva, points: &[u64], archive_queries: &[u64]) -> Value {
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
        &owner.external_publications,
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
pub(crate) fn phy(owner: &mut Phylactery) -> Value {
    let base = common(
        &mut owner.container,
        &owner.memories,
        &owner.external_publications,
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

#[test]
#[ignore = "explicit six-file exhaustive corpus acceptance; requires promoted local fixtures"]
fn canonical_current_corpus_matches_frozen_history_oracle() {
    let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let report: Value = serde_json::from_slice(
        &fs::read(repo.join("benchmarks/canonical-history-equivalence-2026-10-21.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(report["all_owner_domain_mismatches"], 0);
    let root = std::env::var_os("RELIQUARY_CURRENT_CORPUS_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|| repo.join("../reliquary-fixtures/local/authoritative"));
    let fixtures = report["fixtures"].as_array().unwrap();
    assert_eq!(fixtures.len(), 6);
    for row in fixtures {
        let key = row["fixture"].as_str().unwrap();
        let path = root.join(key);
        let original = fs::read(&path).unwrap();
        let modified = fs::metadata(&path).unwrap().modified().unwrap();
        assert_eq!(
            hex(&Sha256::digest(&original)),
            row["canonical_sha256"].as_str().unwrap(),
            "unpromoted or changed fixture {key}"
        );
        let expected = &row["oracle"];
        let actual = if key.ends_with(".rel") {
            let points: Vec<u64> = row["global_projection_points"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_u64().unwrap())
                .collect();
            let archives: Vec<u64> = expected["archive"]["snapshots"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v["cut"].as_u64().unwrap())
                .collect();
            rel(&mut Cva::open(&path).unwrap(), &points, &archives)
        } else {
            phy(&mut Phylactery::open(&path).unwrap())
        };
        if let Some(difference) = mismatch(expected, &actual, "") {
            panic!("current corpus {key}: {difference}");
        }
        assert_eq!(fs::read(&path).unwrap(), original);
        assert_eq!(fs::metadata(&path).unwrap().modified().unwrap(), modified);
        println!("current-format frozen oracle verified {key}");
    }
}
