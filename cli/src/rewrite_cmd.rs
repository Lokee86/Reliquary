use anyhow::{Context, Result, ensure};
use reliquary_memory::rewrite::{ArchiveSelection, RewritePlan, RewriteRequest};
use reliquary_memory::{Phylactery, Reliquary};
use std::{fs, path::Path};

pub fn load_request(
    policy_file: Option<&Path>,
    profiles: &[String],
    include_memories: &[String],
    exclude_memories: &[String],
) -> Result<RewriteRequest> {
    let mut request = if let Some(path) = policy_file {
        let bytes =
            fs::read(path).with_context(|| format!("read rewrite policy {}", path.display()))?;
        serde_json::from_slice(&bytes)
            .with_context(|| format!("parse rewrite policy {}", path.display()))?
    } else {
        RewriteRequest::default()
    };
    if !profiles.is_empty() {
        request.profiles = parse_ids(profiles, "profile")?;
    }
    if !include_memories.is_empty() {
        request.memories.include_ids = parse_ids(include_memories, "Memory")?;
    }
    if !exclude_memories.is_empty() {
        request.memories.exclude_ids = parse_ids(exclude_memories, "Memory")?;
    }
    request.profiles = parse_ids(&request.profiles, "profile")?;
    request.memories.include_ids = parse_ids(&request.memories.include_ids, "Memory")?;
    request.memories.exclude_ids = parse_ids(&request.memories.exclude_ids, "Memory")?;
    Ok(request)
}

pub fn plan_rel(source: &Path, request: &RewriteRequest) -> Result<RewritePlan> {
    let mut rel =
        Reliquary::open(source).with_context(|| format!("open REL source {}", source.display()))?;
    rel.plan_rewrite(request).context("plan REL rewrite")
}

pub fn plan_phy(source: &Path, request: &RewriteRequest) -> Result<RewritePlan> {
    let mut phy = Phylactery::open(source)
        .with_context(|| format!("open PHY source {}", source.display()))?;
    phy.plan_rewrite(request).context("plan PHY rewrite")
}

pub fn print_plan(plan: &RewritePlan) -> Result<()> {
    println!(
        "{}",
        serde_json::to_string_pretty(plan).context("serialize rewrite plan")?
    );
    Ok(())
}

pub fn run_rel(
    source: &Path,
    output: &Path,
    dry_run: bool,
    policy_file: Option<&Path>,
    profiles: &[String],
    include_memories: &[String],
    exclude_memories: &[String],
) -> Result<()> {
    let request = load_request(policy_file, profiles, include_memories, exclude_memories)?;
    let plan = plan_rel(source, &request)?;
    if dry_run {
        return print_plan(&plan);
    }
    ensure!(!output.exists(), "rewrite output already exists");
    let report = reliquary_memory::rewrite::rewrite_from_plan(source, output, &plan)
        .context("execute REL history rewrite")?;
    println!(
        "{}",
        serde_json::to_string_pretty(&report).context("serialize rewrite report")?
    );
    Ok(())
}

pub fn run_phy(
    source: &Path,
    output: &Path,
    dry_run: bool,
    policy_file: Option<&Path>,
    profiles: &[String],
    include_memories: &[String],
    exclude_memories: &[String],
) -> Result<()> {
    let request = load_request(policy_file, profiles, include_memories, exclude_memories)?;
    ensure!(
        request.archive == ArchiveSelection::ProvenanceClosureOnly,
        "PHY rewrite does not accept REL Archive-selection policy"
    );
    let plan = plan_phy(source, &request)?;
    if dry_run {
        return print_plan(&plan);
    }
    ensure!(!output.exists(), "rewrite output already exists");
    let report = reliquary_memory::rewrite::rewrite_from_plan(source, output, &plan)
        .context("execute PHY history rewrite")?;
    println!(
        "{}",
        serde_json::to_string_pretty(&report).context("serialize rewrite report")?
    );
    Ok(())
}

fn parse_ids(values: &[String], kind: &str) -> Result<Vec<String>> {
    let parsed = values
        .iter()
        .map(|value| {
            crate::util::parse_profile_id(value)
                .with_context(|| format!("invalid {kind} id `{value}`"))
                .map(|id| crate::util::hex32(&id.0))
        })
        .collect::<Result<Vec<_>>>()?;
    let unique = parsed.iter().collect::<std::collections::BTreeSet<_>>();
    ensure!(
        unique.len() == parsed.len(),
        "duplicate {kind} selection override"
    );
    Ok(parsed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cli_selection_overrides_replace_policy_file_fields() {
        let mut request = RewriteRequest::default();
        request.profiles = vec!["0".repeat(64)];
        request.memories.include_ids = vec!["1".repeat(64)];
        request.memories.exclude_ids = vec!["2".repeat(64)];
        let bytes = serde_json::to_vec(&request).unwrap();
        let suffix = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::current_dir()
            .unwrap()
            .join(".test-tmp")
            .join(format!("rewrite-policy-{suffix}.json"));
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, bytes).unwrap();
        let loaded = load_request(
            Some(&path),
            &["A".repeat(64)],
            &["B".repeat(64)],
            &["C".repeat(64)],
        )
        .unwrap();
        assert_eq!(loaded.profiles, vec!["a".repeat(64)]);
        assert_eq!(loaded.memories.include_ids, vec!["b".repeat(64)]);
        assert_eq!(loaded.memories.exclude_ids, vec!["c".repeat(64)]);
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn policy_file_rejects_unknown_fields_and_invalid_ids() {
        let suffix = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::current_dir()
            .unwrap()
            .join(".test-tmp")
            .join(format!("rewrite-policy-invalid-{suffix}.json"));
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, br#"{"unexpected":true}"#).unwrap();
        assert!(load_request(Some(&path), &[], &[], &[]).is_err());
        fs::write(&path, br#"{"profiles":["bad-id"]}"#).unwrap();
        assert!(load_request(Some(&path), &[], &[], &[]).is_err());
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn cli_dry_run_for_both_owner_kinds_writes_nothing() {
        use reliquary_memory::{Cva, Phylactery};

        let suffix = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::current_dir()
            .unwrap()
            .join(".test-tmp")
            .join(format!("rewrite-dry-run-{suffix}"));
        fs::create_dir_all(&dir).unwrap();
        let rel_source = dir.join("source.rel");
        let rel_output = dir.join("dry-run-output.rel");
        Cva::create(&rel_source).unwrap().sync().unwrap();
        let rel_hash = fs::read(&rel_source).unwrap();
        run_rel(&rel_source, &rel_output, true, None, &[], &[], &[]).unwrap();
        assert!(!rel_output.exists());
        assert_eq!(fs::read(&rel_source).unwrap(), rel_hash);

        let phy_source = dir.join("source.phy");
        let phy_output = dir.join("dry-run-output.phy");
        Phylactery::create(&phy_source).unwrap().sync().unwrap();
        let phy_hash = fs::read(&phy_source).unwrap();
        run_phy(&phy_source, &phy_output, true, None, &[], &[], &[]).unwrap();
        assert!(!phy_output.exists());
        assert_eq!(fs::read(&phy_source).unwrap(), phy_hash);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn phy_cli_rejects_rel_archive_policy_before_writing() {
        use reliquary_memory::Phylactery;

        let suffix = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::current_dir()
            .unwrap()
            .join(".test-tmp")
            .join(format!("rewrite-phy-option-{suffix}"));
        fs::create_dir_all(&dir).unwrap();
        let source = dir.join("source.phy");
        let output = dir.join("output.phy");
        Phylactery::create(&source).unwrap().sync().unwrap();
        let source_hash = fs::read(&source).unwrap();
        let mut request = RewriteRequest::default();
        request.archive = ArchiveSelection::AllRetained;
        let policy = dir.join("policy.json");
        fs::write(&policy, serde_json::to_vec(&request).unwrap()).unwrap();

        assert!(run_phy(&source, &output, false, Some(&policy), &[], &[], &[]).is_err());
        assert!(!output.exists());
        assert_eq!(fs::read(&source).unwrap(), source_hash);
        fs::remove_dir_all(dir).unwrap();
    }
}
