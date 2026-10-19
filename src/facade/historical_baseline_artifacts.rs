//! Opt-in historical fingerprint baseline generation for authoritative fixtures.

use crate::{Cva, FreshnessEventCut, Phylactery};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeSet,
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
};

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join(".test-tmp")
            .join(format!("historical-baseline-{}", uuid::Uuid::new_v4()));
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

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn fingerprint(fp: &crate::historical_fingerprint::HistoricalFingerprint) -> Value {
    json!({
        "schema": fp.schema,
        "owner_uuid": hex(&fp.owner_uuid),
        "domain": fp.domain,
        "local_cut": fp.local_cut,
        "event_cut": fp.event_cut,
        "rows": fp.rows,
        "sha256": hex(&fp.sha256),
    })
}

fn sample_positions(positions: &[u64]) -> Vec<u64> {
    if positions.is_empty() {
        return Vec::new();
    }
    [0, 1, positions.len() / 2, positions.len() - 1]
        .into_iter()
        .filter_map(|index| positions.get(index).copied())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn family<F>(positions: &[u64], mut at: F) -> Value
where
    F: FnMut(u64) -> Result<crate::historical_fingerprint::HistoricalFingerprint, String>,
{
    let samples = sample_positions(positions)
        .into_iter()
        .map(|cut| {
            let fp = at(cut).unwrap_or_else(|error| panic!("fingerprint at {cut}: {error}"));
            json!({"cut": cut, "fingerprint": fingerprint(&fp)})
        })
        .collect::<Vec<_>>();
    json!({
        "cut_coverage": "sampled",
        "complete_cut_count": positions.len(),
        "sampled_cuts": samples,
    })
}

fn whole_rel_family(cva: &mut Cva, positions: &[u64]) -> Value {
    use crate::historical_whole_owner_fingerprint::{GlobalProjectionError, whole_owner_at};
    let samples: Vec<_> = sample_positions(positions).into_iter().map(|global| {
        match whole_owner_at(cva, global) {
            Ok(projection) => json!({"cut":global,"availability":"available","fingerprint":fingerprint(&projection.fingerprint),
                "selected_local_cuts":{"archive":projection.cuts.archive,"memory":projection.cuts.memory,
                    "entity":projection.cuts.entity,"relationship":projection.cuts.relationship,
                    "graph":projection.cuts.graph,"memory_graph":projection.cuts.memory_graph,
                    "vector_generation":projection.cuts.vector_generation,"ego":projection.ego_version}}),
            Err(GlobalProjectionError::MissingArchiveReference) => json!({"cut":global,"availability":"unavailable",
                "reason":"selected accepted prefix lacks an Archive payload or repository-file binding required by its published records"}),
            Err(error) => panic!("unexpected whole-owner cut {global} error: {error}"),
        }
    }).collect();
    json!({"cut_coverage":"sampled","complete_cut_count":positions.len(),"sampled_cuts":samples})
}

const UNRETAINED_SETTLEMENT_TRAJECTORY: &str =
    "import settlement cut does not retain an intra-settlement trajectory";

fn freshness_family<F>(positions: &[u64], mut at: F) -> Value
where
    F: FnMut(u64) -> Result<crate::historical_fingerprint::HistoricalFingerprint, String>,
{
    let samples = sample_positions(positions)
        .into_iter()
        .map(|cut| match at(cut) {
            Ok(fp) => json!({"cut":cut,"availability":"available","fingerprint":fingerprint(&fp)}),
            Err(error) if error == UNRETAINED_SETTLEMENT_TRAJECTORY => json!({
                "cut":cut,"availability":"unavailable",
                "reason":"accepted import settlement stores only its settled state; no intra-settlement trajectory is retained",
                "source_error":error,
            }),
            Err(error) => panic!("unexpected Freshness cut {cut} error: {error}"),
        })
        .collect::<Vec<_>>();
    json!({
        "cut_coverage":"sampled",
        "complete_cut_count":positions.len(),
        "sampled_cuts":samples,
    })
}

fn freshness_event_family<F>(cuts: &[FreshnessEventCut], mut at: F) -> Value
where
    F: FnMut(
        &FreshnessEventCut,
    ) -> Result<crate::historical_fingerprint::HistoricalFingerprint, String>,
{
    let indices = sample_positions(&(0..cuts.len() as u64).collect::<Vec<_>>());
    let samples = indices
        .into_iter()
        .map(|index| {
            let cut = &cuts[index as usize];
            let metadata = json!({"rel_turn":cut.rel_turn,"event_id_inclusive":cut.event_id_inclusive,
                "graph_version":cut.graph_version});
            match at(cut) {
                Ok(fp) => json!({"cut":metadata,"availability":"available","fingerprint":fingerprint(&fp)}),
                Err(error) if error == UNRETAINED_SETTLEMENT_TRAJECTORY => json!({
                    "cut":metadata,"availability":"unavailable",
                    "reason":"accepted import settlement stores only its settled state; no intra-settlement trajectory is retained",
                    "source_error":error,
                }),
                Err(error) => panic!("unexpected Freshness event cut error: {error}"),
            }
        })
        .collect::<Vec<_>>();
    json!({
        "cut_coverage":"sampled",
        "complete_cut_count":cuts.len(),
        "sampled_cuts":samples,
    })
}

fn copy_and_hash(source: &Path, destination: &Path) -> (u64, String) {
    let before = fs::metadata(source).unwrap();
    let mut input = fs::File::open(source).unwrap();
    let mut output = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(destination)
        .unwrap();
    let mut hash = Sha256::new();
    let mut bytes = 0_u64;
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = input.read(&mut buffer).unwrap();
        if read == 0 {
            break;
        }
        output.write_all(&buffer[..read]).unwrap();
        hash.update(&buffer[..read]);
        bytes += read as u64;
    }
    output.sync_all().unwrap();
    drop(output);
    let after = fs::metadata(source).unwrap();
    assert_eq!(bytes, before.len(), "source length changed during copy");
    assert_eq!(
        before.len(),
        after.len(),
        "source length changed during copy"
    );
    assert_eq!(
        before.modified().unwrap(),
        after.modified().unwrap(),
        "source timestamp changed during copy"
    );
    (bytes, hex(&hash.finalize()))
}

fn source_identity(path: &Path, copied: &(u64, String), expected_kind: &str) -> Value {
    let metadata = fs::metadata(path).unwrap();
    let mut input = fs::File::open(path).unwrap();
    let mut hash = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = input.read(&mut buffer).unwrap();
        if read == 0 {
            break;
        }
        hash.update(&buffer[..read]);
    }
    let digest = hex(&hash.finalize());
    assert_eq!(
        metadata.len(),
        copied.0,
        "fixture size changed after projection"
    );
    assert_eq!(digest, copied.1, "fixture hash changed after projection");
    let mut input = fs::File::open(path).unwrap();
    let mut header = [0_u8; 40];
    input.read_exact(&mut header).unwrap();
    assert_eq!(&header[..8], b"CVA\0\r\n\x1a\n", "fixture header magic");
    assert_eq!(
        u16::from_le_bytes(header[8..10].try_into().unwrap()),
        1,
        "fixture major format"
    );
    assert_eq!(
        u16::from_le_bytes(header[10..12].try_into().unwrap()),
        0,
        "fixture minor format"
    );
    assert_eq!(
        header[16],
        if expected_kind == "phy" { 2 } else { 1 },
        "fixture kind"
    );
    json!({"bytes": copied.0, "sha256": copied.1, "owner_uuid": hex(&header[24..40])})
}

fn memory_revision_digest(
    history: &crate::historical_memory_fingerprint::MemoryHistory,
    records: &[crate::memory_model::MemoryRecord],
) -> Value {
    let mut hash = Sha256::new();
    let mut revisions = 0_u64;
    let mut records: Vec<_> = records.iter().collect();
    records.sort_by_key(|record| (record.id.0, record.revision));
    for record in records {
        let fp = history
            .revision(record.id, record.revision)
            .unwrap_or_else(|error| panic!("Memory revision fingerprint: {error}"));
        hash.update(record.id.0);
        hash.update(record.revision.to_le_bytes());
        hash.update(record.memory_version.to_le_bytes());
        hash.update(fp.sha256);
        revisions += 1;
    }
    json!({"coverage": "all_retained_revisions", "revision_count": revisions,
        "ordered_sha256": hex(&hash.finalize())})
}

fn rel_artifact(cva: &mut Cva, input: Value) -> Value {
    let cuts = crate::historical_cut_enumeration::reliquary_cuts(cva).unwrap();
    let memory_history = crate::historical_memory_fingerprint::MemoryHistory::read(
        &mut cva.container,
        &cva.memories,
    )
    .unwrap();
    let graph_history =
        crate::historical_graph_fingerprint::GraphHistory::read(&cva.container, &cva.graph)
            .unwrap();
    let ego_history =
        crate::historical_ego_fingerprint::EgoHistory::read(&cva.container, &cva.ego).unwrap();
    let vector_history =
        crate::historical_vector_generation_fingerprint::VectorHistory::read(cva).unwrap();
    let archive = cuts.archive.as_ref().unwrap();
    let vectors = cuts.vector_generation.as_ref().unwrap();
    let activity = cuts.freshness_activity.as_ref().unwrap();
    let events = cuts.freshness_events.as_ref().unwrap();
    let mut domains = serde_json::Map::new();
    domains.insert(
        "archive".into(),
        family(archive, |cut| {
            crate::historical_archive_fingerprint::archive_cut(cva, cut)
                .map(|projection| projection.fingerprint)
        }),
    );
    domains.insert(
        "memory".into(),
        family(&cuts.memory, |cut| {
            memory_history
                .at(cut)
                .map(|projection| projection.fingerprint)
        }),
    );
    domains.insert(
        "entity".into(),
        family(&cuts.entity, |cut| {
            crate::historical_entity_fingerprint::entity_cut(&cva.container, &cva.entities, cut)
                .map(|projection| projection.fingerprint)
        }),
    );
    domains.insert(
        "relationship".into(),
        family(&cuts.relationship, |cut| {
            crate::historical_relationship_fingerprint::relationship_cut(
                &cva.container,
                &cva.relationships,
                cut,
            )
            .map(|projection| projection.fingerprint)
        }),
    );
    domains.insert(
        "graph".into(),
        family(&cuts.graph, |cut| {
            graph_history
                .at(cut, false)
                .map(|projection| projection.fingerprint)
        }),
    );
    domains.insert(
        "memory_graph".into(),
        family(&cuts.memory_graph, |cut| {
            graph_history
                .at(cut, true)
                .map(|projection| projection.fingerprint)
        }),
    );
    domains.insert(
        "vector_generation".into(),
        family(vectors, |cut| {
            vector_history
                .at(cut)
                .map(|projection| projection.fingerprint)
        }),
    );
    domains.insert(
        "ego".into(),
        family(&cuts.ego, |cut| {
            ego_history.at(cut).map(|projection| projection.fingerprint)
        }),
    );
    domains.insert(
        "freshness_activity".into(),
        freshness_family(activity, |cut| {
            crate::historical_freshness_fingerprint::freshness_at_turn(cva, cut)
                .map(|projection| projection.fingerprint)
        }),
    );
    domains.insert(
        "freshness_event".into(),
        freshness_event_family(events, |cut| {
            crate::historical_freshness_fingerprint::freshness_at_event(cva, cut)
                .map(|projection| projection.fingerprint)
        }),
    );
    let owner = cva.owner_uuid().unwrap();
    domains.insert("dream_proof_current".into(), json!({"cut_coverage":"current_only",
        "fingerprint":fingerprint(&crate::historical_dream_proof_fingerprint::dream_proof_state(cva).unwrap())}));
    domains.insert("dream_pairs_current".into(), json!({"cut_coverage":"current_only",
        "fingerprint":fingerprint(&crate::historical_dream_pair_fingerprint::reliquary_dream_pairs(cva).unwrap().fingerprint)}));
    domains.insert("dream_processing_current".into(), json!({"cut_coverage":"current_only",
        "fingerprint":fingerprint(&crate::historical_dream_processing_fingerprint::reliquary_dream_processing(cva).unwrap().fingerprint)}));
    let whole = whole_rel_family(cva, &cuts.global);
    let revisions = memory_revision_digest(&memory_history, &cva.memories.records());
    json!({"kind":"rel","owner_uuid":hex(&owner),"input":input,
        "cut_counts":cut_counts(&cuts),"domains":domains,"whole_owner_global":whole,
        "memory_revision_aggregate":revisions})
}

fn phy_artifact(phy: &mut Phylactery, input: Value) -> Value {
    let cuts = crate::historical_cut_enumeration::phylactery_cuts(phy).unwrap();
    let memory_history = crate::historical_memory_fingerprint::MemoryHistory::read(
        &mut phy.container,
        &phy.memories,
    )
    .unwrap();
    let graph = crate::historical_graph_fingerprint::GraphHistory::read(&phy.container, &phy.graph)
        .unwrap();
    let ego =
        crate::historical_ego_fingerprint::EgoHistory::read(&phy.container, &phy.ego).unwrap();
    let mut domains = serde_json::Map::new();
    domains.insert(
        "memory".into(),
        family(&cuts.memory, |cut| {
            memory_history
                .at(cut)
                .map(|projection| projection.fingerprint)
        }),
    );
    domains.insert(
        "entity".into(),
        family(&cuts.entity, |cut| {
            crate::historical_entity_fingerprint::entity_cut(&phy.container, &phy.entities, cut)
                .map(|projection| projection.fingerprint)
        }),
    );
    domains.insert(
        "relationship".into(),
        family(&cuts.relationship, |cut| {
            crate::historical_relationship_fingerprint::relationship_cut(
                &phy.container,
                &phy.relationships,
                cut,
            )
            .map(|projection| projection.fingerprint)
        }),
    );
    domains.insert(
        "graph".into(),
        family(&cuts.graph, |cut| {
            graph
                .at(cut, false)
                .map(|projection| projection.fingerprint)
        }),
    );
    domains.insert(
        "memory_graph".into(),
        family(&cuts.memory_graph, |cut| {
            graph.at(cut, true).map(|projection| projection.fingerprint)
        }),
    );
    domains.insert(
        "ego".into(),
        family(&cuts.ego, |cut| {
            ego.at(cut).map(|projection| projection.fingerprint)
        }),
    );
    domains.insert("dream_pairs_current".into(), json!({"cut_coverage":"current_only",
        "fingerprint":fingerprint(&crate::historical_dream_pair_fingerprint::phylactery_dream_pairs(phy).unwrap().fingerprint)}));
    domains.insert("dream_processing_current".into(), json!({"cut_coverage":"current_only",
        "fingerprint":fingerprint(&crate::historical_dream_processing_fingerprint::phylactery_dream_processing(phy).unwrap().fingerprint)}));
    let whole = family(&cuts.global, |cut| {
        crate::historical_whole_phy_fingerprint::whole_owner_at_phy(phy, cut)
            .map(|projection| projection.fingerprint)
    });
    let owner = phy.owner_uuid().unwrap();
    let revisions = memory_revision_digest(&memory_history, &phy.memories.records());
    json!({"kind":"phy","owner_uuid":hex(&owner),"input":input,
        "cut_counts":cut_counts(&cuts),"domains":domains,"whole_owner_global":whole,
        "memory_revision_aggregate":revisions})
}

fn cut_counts(cuts: &crate::historical_cut_enumeration::HistoricalCutEnumeration) -> Value {
    json!({
        "archive": cuts.archive.as_ref().map(Vec::len),
        "memory": cuts.memory.len(), "entity": cuts.entity.len(),
        "relationship": cuts.relationship.len(), "graph": cuts.graph.len(),
        "memory_graph": cuts.memory_graph.len(),
        "vector_generation": cuts.vector_generation.as_ref().map(Vec::len),
        "ego": cuts.ego.len(), "freshness_activity": cuts.freshness_activity.as_ref().map(Vec::len),
        "freshness_event": cuts.freshness_events.as_ref().map(Vec::len), "global": cuts.global.len(),
    })
}

#[test]
#[cfg(feature = "history-oracle-fixtures")]
fn authoritative_historical_fingerprint_baseline_artifact_is_generated_or_compared() {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let root = PathBuf::from(
        std::env::var_os("RELIQUARY_HISTORY_FIXTURES").expect("RELIQUARY_HISTORY_FIXTURES"),
    );
    let accepted: Value = serde_json::from_slice(
        &fs::read(
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("benchmarks/authoritative-history-rewrite-step62-2026-10-18.json"),
        )
        .unwrap(),
    )
    .unwrap();
    let fixture = Fixture::new();
    let mut owners = Vec::new();
    let mut source_checks = Vec::new();
    for (index, corpus) in [
        "chatgpt-first14d",
        "chatgpt-first28d",
        "rhelm-david-r-ellis-pre-relationship",
    ]
    .iter()
    .enumerate()
    {
        for (kind, name) in [("rel", "project.prj.rel"), ("phy", "user.phy")] {
            let source = root.join(corpus).join(name);
            let copy_path = fixture.path(&format!("{index}.{kind}"));
            let copied = copy_and_hash(&source, &copy_path);
            let input = source_identity(&source, &copied, kind);
            let fixture_key = format!("{corpus}/{name}");
            assert_eq!(
                input["sha256"].as_str(),
                accepted["authoritative_hashes"][fixture_key].as_str(),
                "authoritative fixture bytes differ from accepted Step 62 baseline"
            );
            source_checks.push((source.clone(), copied));
            if kind == "rel" {
                let mut owner = Cva::open(&copy_path).unwrap();
                owners.push(json!({"corpus":corpus,"owner":rel_artifact(&mut owner,input)}));
            } else {
                let mut owner = Phylactery::open(&copy_path).unwrap();
                owners.push(json!({"corpus":corpus,"owner":phy_artifact(&mut owner,input)}));
            }
        }
    }
    for (source, copied) in &source_checks {
        source_identity(
            source,
            copied,
            if source.extension().is_some_and(|ext| ext == "phy") {
                "phy"
            } else {
                "rel"
            },
        );
    }
    let artifact = json!({"schema":1,"coverage":"sampled domain cuts plus complete retained Memory revision digest",
        "authoritative_fixtures":owners});
    let generated = serde_json::to_string_pretty(&artifact).unwrap() + "\n";
    if let Some(output) = std::env::var_os("RELIQUARY_HISTORY_BASELINE_OUT") {
        let output = PathBuf::from(output);
        let output = if output.is_absolute() {
            output
        } else {
            manifest.join(output)
        };
        let repo = manifest.canonicalize().unwrap();
        let parent = output.parent().unwrap().canonicalize().unwrap();
        assert!(
            parent.starts_with(&repo),
            "baseline output must be inside the repository"
        );
        assert!(
            output.file_name().is_some(),
            "baseline output must name a file"
        );
        if output.exists() {
            assert!(
                output.canonicalize().unwrap().starts_with(&repo),
                "existing baseline output must resolve inside the repository"
            );
        }
        fs::write(&output, generated).unwrap();
        eprintln!(
            "wrote authoritative fingerprint baseline: {}",
            output.display()
        );
    } else {
        let expected = manifest.join("benchmarks/historical-owner-baseline-2026-10-19.json");
        let persisted = fs::read_to_string(&expected).unwrap_or_else(|error| panic!(
            "baseline artifact missing at {}; set RELIQUARY_HISTORY_BASELINE_OUT to generate it: {error}",
            expected.display()
        ));
        assert_eq!(
            persisted, generated,
            "historical fingerprint baseline changed"
        );
    }
}
