//! Isolated native Freshness replay over independently enumerated physical prefixes.
use super::*;
use crate::f03_source_witness::{MAX_CUSTODY_BYTES, open_scoped_handle, read_bounded_snapshot};
use std::{fs::OpenOptions, io::Read};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Representative {
    global: u64,
    end_offset: u64,
    activity: u64,
    stage: String,
    members: u64,
    input_chain_sha256: String,
    physical_prefix_sha256: String,
}

#[derive(Deserialize)]
struct SourcePlan {
    copy_id: String,
    source_sha256: String,
    representatives: Vec<Representative>,
}

#[derive(Deserialize)]
struct Plan {
    schema_id: String,
    authentic_admission: bool,
    sources: Vec<SourcePlan>,
}

fn observe_one(copy: &VerifiedCopy, prefix_path: &Path, row: &Representative) -> ToolResult<Value> {
    if row.end_offset < 40
        || row.end_offset > copy.expected.byte_length
        || row.members == 0
        || !matches!(row.stage.as_str(), "birth_only" | "journal")
    {
        return Err("invalid Freshness representative boundary or stage".into());
    }
    let source = open_scoped_handle(&copy.path, false, "Freshness source copy")?;
    if file_identity(&source)? != copy.snapshot.file_identity {
        return Err("Freshness source handle differs from V01 copy".into());
    }
    let mut prefix = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(prefix_path)
        .map_err(|_| "could not create the private Freshness prefix")?;
    let bytes = std::io::copy(&mut source.take(row.end_offset), &mut prefix)
        .map_err(|_| "private Freshness prefix copy failed")?;
    if bytes != row.end_offset {
        return Err("Freshness source prefix has incorrect length".into());
    }
    drop(prefix);
    let held = open_scoped_handle(prefix_path, false, "private Freshness prefix")?;
    let before = hash_file_snapshot(prefix_path)?;
    if before.sha256 != row.physical_prefix_sha256
        || before.byte_length != row.end_offset
        || file_identity(&held)? != before.file_identity
        || physical_link_count(&held)? != 1
    {
        return Err("private Freshness prefix differs from the plan".into());
    }
    let owner = crate::Cva::open_read_only(prefix_path).map_err(|error| {
        format!(
            "Freshness prefix at global {} could not open: {error}",
            row.global
        )
    })?;
    if owner.latest_global_version() > row.global
        || owner.rel_turn_count() != row.activity
        || !snapshots_match(
            &hash_container_handle(&owner.container, before.file_identity)?,
            &before,
        )
    {
        return Err("private Freshness prefix has an unexpected native identity or cut".into());
    }
    let proof = crate::historical_freshness_fingerprint::freshness_at_turn(&owner, row.activity)?
        .fingerprint;
    let observed = json!({
        "global": row.global,
        "physical_global": owner.latest_global_version(),
        "activity": row.activity,
        "stage": row.stage,
        "members": row.members,
        "input_chain_sha256": row.input_chain_sha256,
        "physical_prefix_sha256": row.physical_prefix_sha256,
        "rows": proof.rows,
        "sha256": lower_hex(&proof.sha256),
    });
    drop(owner);
    if !snapshots_match(&hash_file_snapshot(prefix_path)?, &before) {
        return Err("private Freshness prefix changed during native observation".into());
    }
    drop(held);
    fs::remove_file(prefix_path).map_err(|_| "could not retire the private Freshness prefix")?;
    Ok(observed)
}

pub(super) fn observe(
    plan_path: &Path,
    copies: &[VerifiedCopy],
    output: &Path,
) -> ToolResult<Value> {
    ensure_no_reparse_components(plan_path, "Freshness prefix plan")?;
    let (bytes, plan_snapshot, _guard) =
        read_bounded_snapshot(plan_path, MAX_CUSTODY_BYTES, "Freshness prefix plan")?;
    let plan: Plan = serde_json::from_slice(&bytes).map_err(|_| "invalid Freshness prefix plan")?;
    if plan.schema_id != "reliquary-f03-freshness-prefix-reduction-v1"
        || plan.authentic_admission
        || plan.sources.len() != 3
    {
        return Err("Freshness prefix plan must bind the three reviewed RELs".into());
    }
    let mut seen = BTreeSet::new();
    let mut sources = Vec::with_capacity(plan.sources.len());
    let path = output.join("private-freshness-prefix.rel");
    for source in plan.sources {
        let copy = copies
            .iter()
            .find(|copy| {
                copy.source_id == source.copy_id
                    && copy.expected.relative_path.ends_with(".rel")
                    && copy.expected.sha256 == source.source_sha256
            })
            .ok_or("Freshness prefix plan names a different source")?;
        if !seen.insert(source.copy_id.clone())
            || source.representatives.is_empty()
            || source.representatives.len() > 512
            || source.representatives.windows(2).any(|rows| {
                rows[0].global >= rows[1].global || rows[0].end_offset >= rows[1].end_offset
            })
        {
            return Err(
                "Freshness prefix plan has invalid representative ordering or bounds".into(),
            );
        }
        eprintln!(
            "Freshness prefixes for {}: {}",
            copy.source_id,
            source.representatives.len()
        );
        let mut rows = Vec::with_capacity(source.representatives.len());
        for row in source.representatives {
            rows.push(observe_one(copy, &path, &row)?);
        }
        sources.push(json!({
            "copy_id": source.copy_id,
            "source_sha256": source.source_sha256,
            "representatives": rows,
        }));
    }
    if !snapshots_match(&hash_file_snapshot(plan_path)?, &plan_snapshot) {
        return Err("Freshness prefix plan changed during replay".into());
    }
    let exe = env::current_exe().map_err(|_| "producer binary is unavailable")?;
    Ok(json!({
        "status": "PASS",
        "scope": "native Freshness proof at independently selected physical prefix representatives",
        "authentic_admission": false,
        "base_reader_commit": BASE_READER_COMMIT,
        "producer_binary_sha256": hash_file_snapshot(&exe)?.sha256,
        "plan_sha256": plan_snapshot.sha256,
        "sources": sources,
    }))
}
