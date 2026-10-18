use reliquary_memory::{
    Cva, DEFAULT_LEXICAL_WEIGHT, DEFAULT_SEMANTIC_WEIGHT, EmbeddingEndpoint, EmbeddingMode,
    MemoryId, MemoryRetrievalConfig, MemoryRetrievalMode, ModelSwitchboard,
    OpenAiReadyEmbeddingEndpoint, ReliquaryConfig,
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
fn parse_id(v: &str) -> Result<MemoryId, Box<dyn Error>> {
    let mut b = [0u8; 32];
    for i in 0..32 {
        b[i] = u8::from_str_radix(&v[i * 2..i * 2 + 2], 16)?;
    }
    Ok(MemoryId(b))
}
fn hex(id: MemoryId) -> String {
    id.0.iter().map(|b| format!("{b:02x}")).collect()
}
fn rank(ids: &[MemoryId], target: MemoryId) -> Option<usize> {
    ids.iter().position(|id| *id == target).map(|i| i + 1)
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
    for c in value.chars() {
        if c.is_alphanumeric() {
            word.push(c)
        } else {
            flush(&mut word, &mut seen, &mut terms)
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
    let mut matched = 0usize;
    let mut freq = 0usize;
    for term in terms {
        let count = lower.matches(term).count();
        if count > 0 {
            matched += 1;
            freq += count.min(4);
        }
    }
    if matched == 0 {
        return 0.0;
    }
    let coverage = matched as f64 / terms.len() as f64;
    let density = (freq as f64 / (terms.len() * 2) as f64).min(1.0);
    0.85 * coverage + 0.15 * density
}
fn lexical_candidates(cva: &mut Cva, query: &str) -> Result<Vec<(MemoryId, f64)>, Box<dyn Error>> {
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
    hits.sort_by(|a, b| b.1.total_cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    hits.truncate(CANDIDATE_LIMIT);
    Ok(hits)
}
fn hybrid(
    cva: &mut Cva,
    profile: reliquary_memory::CompatibilityProfileId,
    query: &str,
    qv: &[f32],
) -> Result<Vec<MemoryId>, Box<dyn Error>> {
    let lexical = lexical_candidates(cva, query)?;
    let semantic = cva
        .retrieve_memories(
            profile,
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
        .map(|(id, (l, s))| {
            let score = match (
                l.filter(|x| *x > 0.0 && x.is_finite()),
                s.filter(|x| *x > 0.0 && x.is_finite()),
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
    if args.len() != 4 {
        return Err("usage: ellis_memory_retrieval_baseline_live <fixture.rel> <gold.json> <reliquary.cfg> <output.json>".into());
    }
    let fixture = Path::new(&args[0]);
    let gold_path = Path::new(&args[1]);
    let config_path = Path::new(&args[2]);
    let output = Path::new(&args[3]);
    let gold: Gold = serde_json::from_slice(&fs::read(gold_path)?)?;
    if digest(fixture)? != gold.fixture_sha256 {
        return Err("fixture hash mismatch".into());
    }
    let config = ReliquaryConfig::open(config_path)?;
    let route = config
        .models
        .embedding
        .clone()
        .ok_or("embedding route missing")?;
    let route_model = route.model.clone();
    let route_dimensions = route.dimensions;
    let switchboard = ModelSwitchboard::new(config.models, config.credentials)?;
    let endpoint = OpenAiReadyEmbeddingEndpoint::from_switchboard(&switchboard)?;
    let scratch =
        std::env::temp_dir().join(format!("ellis-live-baseline-{}", uuid::Uuid::new_v4()));
    fs::create_dir(&scratch)?;
    let copy = scratch.join("owner.rel");
    fs::copy(fixture, &copy)?;
    #[cfg(windows)]
    {
        let mut p = fs::metadata(&copy)?.permissions();
        p.set_readonly(false);
        fs::set_permissions(&copy, p)?;
    }
    if digest(&copy)? != gold.fixture_sha256 {
        return Err("scratch fixture hash mismatch".into());
    }
    let mut cva = Cva::open(&copy)?;

    const CANONICAL_PROFILE_HEX: &str =
        "2f404c70725ad1162653bf2762c0f5590d0c3f7a1b83c4b9bf87e39091138fa5";
    let profile = cva
        .compatibility_profiles()
        .into_iter()
        .find(|p| {
            p.id.0
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect::<String>()
                == CANONICAL_PROFILE_HEX
        })
        .ok_or("canonical Ellis profile missing")?;
    let compatibility = cva.verify_compatibility_endpoint(profile.id, &endpoint)?;
    let mut evaluated = Vec::new();
    let mut skipped = Vec::new();
    for case in &gold.cases {
        let target = parse_id(&case.gold_memory_id)?;
        if cva
            .memory_body_id(target)
            .ok()
            .and_then(|b| cva.memory_vector_location(profile.id, b))
            .is_some()
        {
            evaluated.push((case, target));
        } else {
            skipped.push(case.id.clone());
        }
    }
    let queries = evaluated
        .iter()
        .map(|(c, _)| c.query.clone())
        .collect::<Vec<_>>();
    let vectors = endpoint.embed(EmbeddingMode::Query, &queries)?;
    if vectors.len() != queries.len() {
        return Err("query embedding count mismatch".into());
    }

    let mut hits = [0usize; 3];
    let mut rr = 0.0;
    let mut rows = Vec::new();
    for ((case, target), qv) in evaluated
        .iter()
        .zip(vectors.iter())
        .map(|((c, t), v)| ((*c, *t), v))
    {
        let ranked = hybrid(&mut cva, profile.id, &case.query, qv)?;
        let r = rank(&ranked, target);
        for (i, k) in [1usize, 5, 10].into_iter().enumerate() {
            hits[i] += usize::from(r.is_some_and(|x| x <= k));
        }
        rr += r
            .filter(|x| *x <= RESULT_LIMIT)
            .map(|x| 1.0 / x as f64)
            .unwrap_or(0.0);
        rows.push(
            json!({"id":case.id,"query":case.query,"gold_memory_id":hex(target),"rank":r,
            "top10":ranked.iter().take(RESULT_LIMIT).copied().map(hex).collect::<Vec<_>>() }),
        );
    }
    let n = evaluated.len() as f64;
    let report = json!({
        "version":1,"fixture_sha256":gold.fixture_sha256,"gold_cases":gold.cases.len(),"evaluated_cases":evaluated.len(),"skipped_cases":skipped,
        "method":"Live production Query embeddings against canonical retained Ellis profile 2f404c...; original reconstructed Memory hybrid baseline: top-30 lexical + top-30 GlobalExact semantic, 0.45/0.55 fusion with undiluted single-lane scores, top-10. Current endpoint compatibility is recorded but not used as an acceptance gate for this diagnostic because the route is currently drifting below policy threshold.",
        "embedding":{"model":route_model,"configured_dimensions":route_dimensions,"profile_dimensions":profile.dimensions,
            "profile_id":profile.id.0.iter().map(|b|format!("{b:02x}")).collect::<String>(),"compatibility_passed":compatibility.compatible,"compatibility_min_cosine":compatibility.minimum_cosine},
        "baseline":{"hits_at_1_5_10":hits,"rates_at_1_5_10":hits.map(|x|x as f64/n),"mrr_at_10":rr/n},
        "results":rows
    });
    fs::write(output, serde_json::to_vec_pretty(&report)?)?;
    if digest(fixture)? != gold.fixture_sha256 {
        return Err("authoritative fixture changed".into());
    }
    let _ = fs::remove_dir_all(scratch);
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}
