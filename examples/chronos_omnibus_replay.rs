use serde_json::{Value, json};
use std::{error::Error, fs, path::PathBuf};
fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() != 2 {
        return Err("usage: chronos_omnibus_replay <cases.jsonl> <output.json>".into());
    }
    let mut rows = Vec::new();
    for line in fs::read_to_string(&args[0])?
        .lines()
        .filter(|line| !line.trim().is_empty())
    {
        let case: Value = serde_json::from_str(line)?;
        let text = format!(
            "{}\n{}",
            case["title"].as_str().unwrap_or(""),
            case["content"].as_str().unwrap_or("")
        );
        let choices = reliquary_chronos::chronos_choices::assess_choices(
            &text,
            case["source_time_ns"].as_i64(),
        );
        rows.push(json!({"memory_id":case["memory_id"], "unresolved":choices.assessment.resolution.unresolved_indications.len(), "candidate_sets":choices.sets.iter().map(|set| json!({"evidence":set.indication.evidence, "candidates":set.candidates.iter().map(|c| &c.canonical_expression).collect::<Vec<_>>()})).collect::<Vec<_>>() }));
    }
    if let Some(parent) = PathBuf::from(&args[1]).parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent)?;
        }
    }
    fs::write(&args[1], serde_json::to_vec_pretty(&rows)?)?;
    Ok(())
}
