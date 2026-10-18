//! Deterministic Ellis Memory retrieval comparison. No provider/model inference.
//! Semantic queries replay stored vectors from separately labeled related Memories.
use reliquary_memory::{
    Cva, DEFAULT_LEXICAL_WEIGHT, DEFAULT_SEMANTIC_WEIGHT, MemoryId, MemoryRetrievalConfig,
    MemoryRetrievalMode, ScalarType,
};
use serde::Deserialize;
use serde_json::json;
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, HashSet},
    error::Error,
    fs,
    io::Read,
    path::Path,
};

const CANDIDATE_LIMIT: usize = 30;
const RESULT_LIMIT: usize = 10;

#[derive(Deserialize)]
struct Gold {
    fixture_sha256: String,
    cases: Vec<Case>,
}
#[derive(Deserialize)]
struct Case {
    id: String,
    query: String,
    gold_memory_id: String,
    query_vector_source_id: String,
}

fn digest(path: &Path) -> Result<String, Box<dyn Error>> {
    let mut f = fs::File::open(path)?;
    let mut h = Sha256::new();
    let mut b = [0u8; 65536];
    loop {
        let n = f.read(&mut b)?;
        if n == 0 {
            break;
        }
        h.update(&b[..n]);
    }
    Ok(format!("{:x}", h.finalize()))
}
fn parse_id(value: &str) -> Result<MemoryId, Box<dyn Error>> {
    if value.len() != 64 {
        return Err("MemoryId must be 64 hex chars".into());
    }
    let mut bytes = [0u8; 32];
    for i in 0..32 {
        bytes[i] = u8::from_str_radix(&value[i * 2..i * 2 + 2], 16)?;
    }
    Ok(MemoryId(bytes))
}
fn hex(id: MemoryId) -> String {
    id.0.iter().map(|b| format!("{b:02x}")).collect()
}
fn vector(
    cva: &mut Cva,
    profile: reliquary_memory::CompatibilityProfileId,
    id: MemoryId,
) -> Result<Vec<f32>, Box<dyn Error>> {
    let body = cva.memory_body_id(id)?;
    let loc = cva
        .memory_vector_location(profile, body)
        .ok_or("missing Memory vector")?;
    let set = cva.memory_vectors(loc.set_id)?;
    let packed = cva.packed_vectors(set.packed_vector_id)?;
    if packed.schema().scalar != ScalarType::F32 {
        return Err("non-f32 Memory vector".into());
    }
    let row = packed
        .row(usize::try_from(loc.row)?)
        .ok_or("missing Memory vector row")?;
    let out = row
        .chunks_exact(4)
        .map(|x| f32::from_le_bytes(x.try_into().unwrap()))
        .collect::<Vec<_>>();
    if out.iter().any(|x| !x.is_finite()) {
        return Err("non-finite Memory vector".into());
    }
    Ok(out)
}
fn rank_of(ids: &[MemoryId], target: MemoryId) -> Option<usize> {
    ids.iter().position(|id| *id == target).map(|i| i + 1)
}
fn hit(rank: Option<usize>, k: usize) -> bool {
    rank.is_some_and(|r| r <= k)
}
fn lexical_terms(value: &str) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut terms = Vec::new();
    let mut word = String::new();
    let flush = |word: &mut String, seen: &mut HashSet<String>, terms: &mut Vec<String>| {
        if word.is_empty() {
            return;
        }
        let term = word.to_lowercase();
        word.clear();
        if seen.insert(term.clone()) {
            terms.push(term);
        }
    };
    for current in value.chars() {
        if current.is_alphanumeric() {
            word.push(current);
        } else {
            flush(&mut word, &mut seen, &mut terms);
        }
    }
    flush(&mut word, &mut seen, &mut terms);
    terms
}

fn lexical_score(text: &str, terms: &[String]) -> f64 {
    if terms.is_empty() {
        return 0.0;
    }
    let lower = text.to_lowercase();
    let mut matched_terms = 0usize;
    let mut frequency = 0usize;
    for term in terms {
        let count = lower.matches(term).count();
        if count > 0 {
            matched_terms += 1;
            frequency += count.min(4);
        }
    }
    if matched_terms == 0 {
        return 0.0;
    }
    let coverage = matched_terms as f64 / terms.len() as f64;
    let density = (frequency as f64 / (terms.len() * 2) as f64).min(1.0);
    0.85 * coverage + 0.15 * density
}

fn memory_lexical_candidates(
    cva: &mut Cva,
    query: &str,
) -> Result<Vec<(MemoryId, f64)>, Box<dyn Error>> {
    let terms = lexical_terms(query);
    let mut hits = Vec::new();
    for id in cva.memory_ids() {
        let memory = cva.memory(id)?;
        if memory.archived {
            continue;
        }
        let score = lexical_score(&format!("{}\n\n{}", memory.title, memory.content), &terms);
        if score > 0.0 {
            hits.push((id, score));
        }
    }
    hits.sort_by(|left, right| {
        right
            .1
            .total_cmp(&left.1)
            .then_with(|| left.0.cmp(&right.0))
    });
    hits.truncate(CANDIDATE_LIMIT);
    Ok(hits)
}

fn hybrid(
    cva: &mut Cva,
    index: &reliquary_memory::MemoryRetrievalIndex,
    query: &str,
    qv: &[f32],
) -> Result<Vec<MemoryId>, Box<dyn Error>> {
    let lexical = memory_lexical_candidates(cva, query)?;
    let semantic = cva
        .retrieve_memories_with_index(
            index,
            qv,
            MemoryRetrievalConfig {
                mode: MemoryRetrievalMode::GlobalExact,
                seed_limit: CANDIDATE_LIMIT,
                traversal_budget: CANDIDATE_LIMIT,
                ..MemoryRetrievalConfig::default()
            },
        )?
        .seeds;
    let mut evidence = BTreeMap::<MemoryId, (Option<f64>, Option<f64>)>::new();
    for (id, score) in lexical {
        evidence.entry(id).or_default().0 = Some(score);
    }
    for h in semantic {
        evidence.entry(h.memory_id).or_default().1 = Some(h.score);
    }
    let mut ranked = evidence
        .into_iter()
        .map(|(id, (lex, sem))| {
            let score = match (
                lex.filter(|x| *x > 0.0 && x.is_finite()),
                sem.filter(|x| *x > 0.0 && x.is_finite()),
            ) {
                (Some(l), Some(s)) => DEFAULT_LEXICAL_WEIGHT * l + DEFAULT_SEMANTIC_WEIGHT * s,
                (Some(l), None) => l,
                (None, Some(s)) => s,
                _ => 0.0,
            };
            (id, score)
        })
        .collect::<Vec<_>>();
    ranked.sort_by(|a, b| b.1.total_cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    Ok(ranked.into_iter().map(|x| x.0).collect())
}

fn main() -> Result<(), Box<dyn Error>> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    if !(3..=5).contains(&args.len()) {
        return Err(
            "usage: ellis_memory_retrieval_compare <fixture.rel> <gold.json> <output.json> [profile-id] [expected-fixture-sha256]".into(),
        );
    }
    let fixture = Path::new(&args[0]);
    let gold_path = Path::new(&args[1]);
    let output = Path::new(&args[2]);
    let gold: Gold = serde_json::from_slice(&fs::read(gold_path)?)?;
    let expected_hash = args.get(4).unwrap_or(&gold.fixture_sha256);
    if digest(fixture)? != *expected_hash {
        return Err("fixture hash mismatch".into());
    }
    let scratch = std::env::temp_dir().join(format!("ellis-retrieval-{}", uuid::Uuid::new_v4()));
    fs::create_dir(&scratch)?;
    let copy = scratch.join("owner.rel");
    fs::copy(fixture, &copy)?;
    #[cfg(windows)]
    {
        let mut p = fs::metadata(&copy)?.permissions();
        p.set_readonly(false);
        fs::set_permissions(&copy, p)?;
    }
    let mut cva = Cva::open(&copy)?;
    let community_snapshot = cva.refresh_communities_leiden()?;
    let community_count = community_snapshot.communities.len();
    let parsed = gold
        .cases
        .iter()
        .map(|c| {
            Ok((
                c,
                parse_id(&c.gold_memory_id)?,
                parse_id(&c.query_vector_source_id)?,
            ))
        })
        .collect::<Result<Vec<_>, Box<dyn Error>>>()?;
    let mut profiles = cva.active_memory_compatibility_profiles();
    if let Some(requested) = args.get(3) {
        profiles.retain(|p| {
            p.id.0
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect::<String>()
                == *requested
        });
    }
    if profiles.len() != 1 {
        return Err("benchmark requires one unambiguous complete operational profile".into());
    }
    let profile = profiles.remove(0);
    let coverage = parsed
        .iter()
        .filter(|(_, gold, surrogate)| {
            cva.memory_body_id(*gold)
                .ok()
                .and_then(|b| cva.memory_vector_location(profile.id, b))
                .is_some()
                && cva
                    .memory_body_id(*surrogate)
                    .ok()
                    .and_then(|b| cva.memory_vector_location(profile.id, b))
                    .is_some()
        })
        .count();
    if coverage != parsed.len() {
        return Err(format!("selected profile covers {coverage}/{} cases", parsed.len()).into());
    }
    let index = cva.build_memory_retrieval_index(
        profile.id,
        MemoryRetrievalConfig::default().subcentroids_per_community,
    )?;
    let mut rows = Vec::new();
    let mut h_hits = [0usize; 3];
    let mut c_hits = [0usize; 3];
    let mut h_rr = 0.0;
    let mut c_rr = 0.0;
    let mut routed = 0usize;
    let mut fallback = 0usize;
    for (case, target, surrogate) in parsed {
        let qv = vector(&mut cva, profile.id, surrogate)?;
        let hybrid_ids = hybrid(&mut cva, &index, &case.query, &qv)?;
        let current =
            cva.retrieve_memories_with_index(&index, &qv, MemoryRetrievalConfig::default())?;
        let current_ids = current.memories.clone();
        if current.mode_used == MemoryRetrievalMode::CommunityRouted {
            routed += 1
        } else {
            fallback += 1
        }
        let hr = rank_of(&hybrid_ids, target);
        let cr = rank_of(&current_ids, target);
        for (i, k) in [1usize, 5, 10].into_iter().enumerate() {
            h_hits[i] += usize::from(hit(hr, k));
            c_hits[i] += usize::from(hit(cr, k));
        }
        h_rr += hr
            .filter(|r| *r <= RESULT_LIMIT)
            .map(|r| 1.0 / r as f64)
            .unwrap_or(0.0);
        c_rr += cr
            .filter(|r| *r <= RESULT_LIMIT)
            .map(|r| 1.0 / r as f64)
            .unwrap_or(0.0);
        rows.push(json!({
            "id":case.id,"query":case.query,"gold_memory_id":hex(target),"query_vector_source_id":hex(surrogate),
            "hybrid":{"rank":hr,"top10":hybrid_ids.iter().take(RESULT_LIMIT).copied().map(hex).collect::<Vec<_>>()},
            "current":{"rank":cr,"mode":format!("{:?}",current.mode_used),"used_global_fallback":current.used_global_fallback,
                "selected_communities":current.selected_communities.len(),"routing_vectors_scored":current.routing_vectors_scored,
                "memory_vectors_scored":current.memory_vectors_scored,"top10":current_ids.iter().take(RESULT_LIMIT).copied().map(hex).collect::<Vec<_>>()}
        }));
    }
    let n = gold.cases.len() as f64;
    let report = json!({
        "version":1,"fixture_sha256":expected_hash,"gold_fixture_sha256":gold.fixture_sha256,"profile":profile.id.0.iter().map(|b|format!("{b:02x}")).collect::<String>(),
        "dimensions":profile.dimensions,"cases":gold.cases.len(),"community_count":community_count,
        "method":"No inference. Natural-language lexical query plus stored vector from a distinct manually selected related Memory. Hybrid = top-30 Memory lexical + top-30 global exact semantic, original 0.45/0.55 fusion with undiluted single-lane scores, top-10. Current = default MemoryRetrievalConfig community-routed semantic seeds + graph traversal, evaluated at top-10.",
        "hybrid":{"hits_at_1_5_10":h_hits,"rates_at_1_5_10":h_hits.map(|x|x as f64/n),"mrr_at_10":h_rr/n},
        "current":{"hits_at_1_5_10":c_hits,"rates_at_1_5_10":c_hits.map(|x|x as f64/n),"mrr_at_10":c_rr/n,
            "community_routed_cases":routed,"global_fallback_cases":fallback},
        "results":rows
    });
    fs::write(output, serde_json::to_vec_pretty(&report)?)?;
    if digest(fixture)? != *expected_hash {
        return Err("authoritative fixture changed".into());
    }
    let _ = fs::remove_dir_all(scratch);
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}
