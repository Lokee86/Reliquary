//! Disposable N5 inspector: ordinary owners, no writes or compatibility authority.
use super::*;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
#[path = "fixture_normalization_publication.rs"]
mod publication;

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
fn proof(value: &impl std::fmt::Debug) -> String {
    hex(&Sha256::digest(format!("{value:?}").as_bytes()))
}
fn provenance(m: &crate::Memory) -> String {
    proof(&(
        &m.source_node_id,
        &m.content_source_conversation_id,
        &m.content_source_node_id,
        &m.grounding_source_conversation_id,
        &m.grounding_source_node_id,
        &m.source_episode_id,
        &m.source_time_ns,
        &m.source_ref,
        &m.routing_metadata,
    ))
}
fn rel_snapshot(path: &Path) -> Value {
    let digest = file_digest(path).unwrap();
    let mut cva = Cva::open(path).unwrap();
    let mut nodes = BTreeMap::new();
    for node in cva.archive.nodes.iter() {
        nodes.insert(serde_json::to_string(&(&node.conversation_id, &node.id)).unwrap(),
            json!({"binding":proof(node), "position":cva.activity_position_for_turn(&node.conversation_id, &node.id)}));
    }
    let mut branches = BTreeMap::new();
    for branch in cva.branches() {
        branches.insert(
            serde_json::to_string(&(&branch.conversation_id, &branch.id)).unwrap(),
            proof(&branch),
        );
    }
    let mut memories = BTreeMap::new();
    let mut history = BTreeMap::new();
    for id in cva.memory_ids() {
        let m = cva.memory(id).unwrap();
        memories.insert(
            hex(&id.0),
            json!({"body":hex(&cva.memory_body_id(id).unwrap().0),
            "provenance":provenance(&m), "revision":m.revision}),
        );
        for revision in 1..=m.revision {
            history.insert(
                format!("{}/{revision}", hex(&id.0)),
                proof(&cva.memory_revision(id, revision).unwrap()),
            );
        }
    }
    let actual = cva.historical_completed_initial_dream_passes().unwrap();
    let publication = publication::inspect(&mut cva);
    let publication_births: BTreeSet<_> = cva
        .memory_ids()
        .iter()
        .filter(|id| cva.freshness_is_publication_birth(**id))
        .map(|id| hex(&id.0))
        .collect();
    let actual_proofs: BTreeMap<_, _> = actual
        .iter()
        .map(|(id, pass)| {
            let (roots, _) = cva.initial_dream_committed_evidence(pass).unwrap();
            assert_eq!(roots, pass.roots);
            (hex(&id.0), proof(pass))
        })
        .collect();
    let generation = cva
        .vector_generations
        .generations()
        .iter()
        .max_by_key(|g| g.vector_version)
        .copied();
    let mut eligible = 0usize;
    let mut usable = 0usize;
    for fragment in cva.fragments() {
        if cva
            .semantic_fragment_position(fragment.id)
            .unwrap()
            .is_some()
        {
            eligible += 1;
            if let Some(g) = generation {
                if let crate::cva_semantic_source::SemanticSourceReadiness::Ready(source) =
                    cva.prepare_semantic_fragment(fragment.id, g.id).unwrap()
                {
                    usable += usize::from(!source.vectors.is_empty());
                }
            }
        }
    }
    let target_bindings = generation.map_or(0, |g| {
        cva.memory_ids()
            .iter()
            .filter(|id| {
                cva.memory_vector_location(
                    g.compatibility_profile_id,
                    cva.memory_body_id(**id).unwrap(),
                )
                .is_some()
            })
            .count()
    });
    let vectors = json!({"profile":generation.map(|g|hex(&g.compatibility_profile_id.0)),
        "generation":generation.map(|g|hex(&g.id.0)),"eligible_fragments":eligible,
        "usable_fragments":usable,"current_target_bindings":target_bindings,
        "ready":eligible>0 && eligible==usable && target_bindings==memories.len()});
    let mut preflight = json!({"ready":false});
    match cva
        .freeze_historical_import()
        .and_then(|frozen| frozen.prepare_native())
    {
        Ok(_) => preflight = json!({"ready":true}),
        Err(error) => preflight["error"] = json!(error.to_string()),
    }
    let result = json!({"owner":hex(&cva.owner_uuid().unwrap()), "activity_count":cva.rel_turn_count(),
        "nodes":nodes,"branches":branches,"memories":memories,"history":history,
        "graph":cva.semantic_graph_relations().iter().map(proof).collect::<BTreeSet<_>>(),
        "episodes":cva.episodes().iter().map(proof).collect::<BTreeSet<_>>(),
        "fragments":cva.fragments().iter().map(proof).collect::<BTreeSet<_>>(),
        "accepted_use":cva.freshness.fixture_normalization_use_proofs().unwrap(),
        "actual_dream":actual_proofs,"preflight":preflight,
        "archive_clock_valid":cva.historical_freshness_timeline().is_ok(),
        "checkpoint":cva.freshness.import_checkpoint().is_some(),"vectors":vectors,
        "publication":publication,"publication_births":publication_births,
        "versions":{"global":cva.latest_global_version(),"archive":cva.archive_version(),
            "memory":cva.memory_version(),"graph":cva.graph_version()}});
    drop(cva);
    assert_eq!(
        file_digest(path).unwrap(),
        digest,
        "inspection must not change candidate bytes"
    );
    result
}
fn phy_snapshot(path: &Path) -> Value {
    let digest = file_digest(path).unwrap();
    let mut phy = crate::Phylactery::open(path).unwrap();
    let mut memories = BTreeMap::new();
    let mut history = BTreeMap::new();
    for id in phy.memory_ids() {
        let m = phy.memory(id).unwrap();
        memories.insert(
            hex(&id.0),
            json!({"body":hex(&phy.memory_body_id(id).unwrap().0),
            "provenance":provenance(&m),"revision":m.revision}),
        );
        for revision in 1..=m.revision {
            history.insert(
                format!("{}/{revision}", hex(&id.0)),
                proof(&phy.memory_revision(id, revision).unwrap()),
            );
        }
    }
    let result = json!({"owner":hex(&phy.owner_uuid().unwrap()),"memories":memories,"history":history,
        "graph":phy.semantic_graph_relations().iter().map(proof).collect::<BTreeSet<_>>(),
        "versions":{"global":phy.latest_global_version(),"memory":phy.memory_version(),"graph":phy.graph_version()}});
    drop(phy);
    assert_eq!(
        file_digest(path).unwrap(),
        digest,
        "inspection must not change candidate bytes"
    );
    result
}

#[test]
#[ignore = "disposable exact-fixture N5 probe; invoked by scripts/freshness_fixture_normalizer.py"]
fn fixture_normalization_owner_probe() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let request = PathBuf::from(std::env::var_os("RELIQUARY_N5_REQUEST").unwrap());
    let job: Value = serde_json::from_slice(&std::fs::read(&request).unwrap()).unwrap();
    let fixture = job["fixture"].as_str().unwrap();
    let (rel_owner, phy_owner, rel_hash, phy_hash) = match fixture {
        "chatgpt-first14d" => (
            "73eff05fe65e4fb4b430f9af2237b48b",
            "ce8c8fc9640d4e76abd8dab60424763b",
            "49751a62dd8644800d19976a0696a69366dc1e4f8d7c39bb4b69fbd7a8a5d18b",
            "344f1cba2a759a339fe8959135a075210851f236cc9652246ac924320a92c12c",
        ),
        "chatgpt-first28d" => (
            "73eff05fe65e4fb4b430f9af2237b48b",
            "ce8c8fc9640d4e76abd8dab60424763b",
            "8f4a2bf871d7f5368edde93954a2260a29c86131b28475b323a7b6c70f8c51e4",
            "a095352cfac556e04cb2ad6f3258ced5aacecbf88429a6deb36c7659dfc715cd",
        ),
        "rhelm-david-r-ellis-pre-relationship" => (
            "41fd7fac75d54d05899c0dce0823aa36",
            "4ad56e4aecc4468f8742eaa343efd9e7",
            "e53255ff5258aaeb5fc4437e5993613868adf646d9e5f711fe2b079f7a340fd4",
            "110301f1f12873b8ac1a2eb5000a004fdcd0287d7e141622e6cd18e7b2afe937",
        ),
        _ => panic!("fixture is not allowlisted"),
    };
    let work = root.join("target/freshness-normalization").join(fixture);
    assert_eq!(
        request.canonicalize().unwrap(),
        work.join("request.json").canonicalize().unwrap()
    );
    let mode = job["mode"].as_str().unwrap();
    if mode == "inspect" {
        assert_eq!(job["rel_sha256"], rel_hash);
        assert_eq!(job["phy_sha256"], phy_hash);
    }
    let base = match mode {
        "inspect" => work.join("inspection"),
        "verify" => root
            .parent()
            .unwrap()
            .join("reliquary-fixtures/local/authoritative")
            .join(fixture),
        _ => panic!("unsupported fixture probe mode"),
    };
    let rel = base.join(if mode == "inspect" {
        "project.prj.rel"
    } else {
        "project.prj.rel.n5-candidate"
    });
    let phy = base.join(if mode == "inspect" {
        "user.phy"
    } else {
        "user.phy.n5-candidate"
    });
    for (path, key) in [(&rel, "rel_sha256"), (&phy, "phy_sha256")] {
        assert_eq!(hex(&file_digest(path).unwrap()), job[key].as_str().unwrap());
        assert!(
            !std::fs::symlink_metadata(path)
                .unwrap()
                .file_type()
                .is_symlink()
        );
    }
    let rel = rel_snapshot(&rel);
    let phy = phy_snapshot(&phy);
    assert_eq!(rel["owner"], rel_owner);
    assert_eq!(phy["owner"], phy_owner);
    std::fs::write(
        work.join("probe.json"),
        serde_json::to_vec_pretty(&json!({"schema":1,"rel":rel,"phy":phy})).unwrap(),
    )
    .unwrap();
}

#[test]
fn fixture_normalization_native_probe_preserves_current_owner_and_rejects_incomplete_h1() {
    let (path, mut cva, id) = super::historical_import_fixture("n5-probe.rel");
    super::historical_events::vectors(&mut cva);
    let before = rel_snapshot(&path);
    assert_eq!(before["preflight"]["ready"], true);
    assert_eq!(before["actual_dream"].as_object().unwrap().len(), 1);
    cva.append_node(
        "tail".into(),
        "n5".into(),
        None,
        "user".into(),
        -1,
        "Unprocessed source",
    )
    .unwrap();
    cva.sync().unwrap();
    drop(cva);
    let after = rel_snapshot(&path);
    assert_eq!(after["preflight"]["ready"], false);
    assert_eq!(
        before["memories"][hex(&id.0)],
        after["memories"][hex(&id.0)]
    );
}
