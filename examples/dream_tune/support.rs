use reliquary_memory::{CompatibilityProfileId, Cva, DreamCandidateConfig};
use serde_json::{Value, json};
use std::collections::HashSet;
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

pub fn prepare_run_rel(baseline_path: &Path, output_dir: &Path) -> Result<PathBuf, Box<dyn Error>> {
    fs::create_dir_all(output_dir)?;
    let baseline = Cva::open(baseline_path)?;
    drop(baseline);

    let run_path = output_dir.join("dream-run.rel");
    if run_path.exists() {
        return Err(format!("run REL already exists: {}", run_path.display()).into());
    }
    fs::copy(baseline_path, &run_path)?;
    Ok(run_path)
}

pub fn single_memory_profile(cva: &Cva) -> Result<CompatibilityProfileId, Box<dyn Error>> {
    let profiles: HashSet<_> = cva
        .memory_vector_infos()
        .into_iter()
        .map(|info| info.compatibility_profile_id)
        .collect();
    if profiles.len() != 1 {
        return Err(format!(
            "expected exactly one Memory vector profile, found {}",
            profiles.len()
        )
        .into());
    }
    Ok(*profiles.iter().next().expect("one profile"))
}

#[allow(clippy::too_many_arguments)]
pub fn write_partial_report(
    output_dir: &Path,
    baseline_path: &Path,
    run_path: &Path,
    classifier_model: &str,
    verifier_model: &str,
    candidate_config: DreamCandidateConfig,
    verification: &str,
    baseline: &Value,
    runs: &[Value],
    recoverable_failures: usize,
    fatal_error: Option<String>,
) -> Result<(), Box<dyn Error>> {
    write_json(
        &output_dir.join("report.partial.json"),
        &json!({
            "format": "dream-tuning-v1",
            "baseline_cva": baseline_path,
            "run_cva": run_path,
            "classifier_model": classifier_model,
            "verifier_model": verifier_model,
            "candidate_config": candidate_config_json(candidate_config),
            "verification_policy": verification,
            "recoverable_failures": recoverable_failures,
            "fatal_error": fatal_error,
            "baseline": baseline,
            "runs": runs,
        }),
    )
}

pub fn write_gold_template(output_dir: &Path, report: &Value) -> Result<(), Box<dyn Error>> {
    let memories = report["baseline"]["memories"].clone();
    write_json(
        &output_dir.join("gold-template.json"),
        &json!({
            "format": "dream-gold-v1",
            "source_cva": report["baseline_cva"],
            "memories": memories,
            "pairs": [],
            "lifecycle": [],
        }),
    )
}

pub fn candidate_config_json(config: DreamCandidateConfig) -> Value {
    json!({
        "limit": config.limit,
        "semantic_limit": config.semantic_limit,
        "prior_semantic_quota": config.prior_semantic_quota,
        "lexical_limit": config.lexical_limit,
        "temporal_limit": config.temporal_limit,
    })
}

pub fn write_json(path: &Path, value: &Value) -> Result<(), Box<dyn Error>> {
    fs::write(path, serde_json::to_vec_pretty(value)?)?;
    Ok(())
}

pub fn option_usize(args: &[String], name: &str) -> Result<Option<usize>, Box<dyn Error>> {
    match option_string(args, name) {
        Some(value) => Ok(Some(value.parse()?)),
        None => Ok(None),
    }
}

pub fn option_string<'a>(args: &'a [String], name: &str) -> Option<&'a str> {
    args.windows(2)
        .find(|pair| pair[0] == name)
        .map(|pair| pair[1].as_str())
}
