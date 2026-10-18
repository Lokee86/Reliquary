//! Copy-only retained REL normalization. Source artifacts are never opened for mutation.
use anyhow::{Context, Result, ensure};
use reliquary_memory::owner_lifecycle::LifecycleTarget;
use reliquary_memory::{CompatibilityProfileId, Cva, HistoryPruneRequest};
use std::{
    collections::BTreeSet,
    fs::{self, File, OpenOptions},
    path::Path,
};

pub fn run(
    source: &Path,
    output: &Path,
    keep: &str,
    prune_retired: bool,
    reclaim: bool,
) -> Result<()> {
    let keep = crate::util::parse_profile_id(keep)?;
    ensure!(!output.exists(), "repair output already exists");
    let mut input = File::open(source).context("open retained source")?;
    let mut copied = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(output)
        .context("create disposable repair copy")?;
    let result = (|| -> Result<()> {
        std::io::copy(&mut input, &mut copied)?;
        copied.sync_all()?;
        drop(copied);
        let mut rel = Cva::open(output)?;
        let before = Snapshot::read(&mut rel)?;
        rel.compatibility_profile(keep)
            .context("requested canonical profile is absent")?;
        ensure!(
            !rel.compatibility_profile_is_retired(keep),
            "canonical profile was already retired"
        );
        ensure!(
            rel.current_vector_generation(keep).is_some(),
            "canonical Archive generation is not complete/current"
        );
        if rel.memory_stats().memories != 0 {
            ensure!(
                rel.active_memory_compatibility_profile(keep).is_some(),
                "canonical Memory vectors do not cover current bodies"
            );
        }
        let migrated = rel.migrate_legacy_import_framing()?;
        let mut retired = Vec::new();
        for profile in rel.compatibility_profiles() {
            if profile.id != keep && rel.retire_compatibility_profile(profile.id)? {
                retired.push(crate::util::hex32(&profile.id.0));
            }
        }
        rel.sync()?;
        let after = Snapshot::read(&mut rel)?;
        ensure!(
            before == after,
            "repair changed semantic/version/profile identity state"
        );
        verify_active(&rel, keep)?;
        drop(rel);
        let mut reopened = Cva::open(output)?;
        ensure!(
            after == Snapshot::read(&mut reopened)?,
            "reopen changed repaired state"
        );
        verify_active(&reopened, keep)?;
        drop(reopened);
        if prune_retired {
            let scratch = output.with_extension("repair-prune.rel");
            ensure!(!scratch.exists(), "prune scratch path already exists");
            let prune_result = (|| -> Result<()> {
                let mut rel = Cva::open(output)?;
                let targets = rel
                    .compatibility_profiles()
                    .into_iter()
                    .filter(|profile| rel.compatibility_profile_is_retired(profile.id))
                    .map(|profile| LifecycleTarget::CompatibilityProfile(profile.id))
                    .collect::<Vec<_>>();
                ensure!(
                    !targets.is_empty(),
                    "no retired profiles selected for pruning"
                );
                rel.prune_tombstoned_history(&scratch, HistoryPruneRequest { targets })?;
                let mut pruned = Cva::open(&scratch)?;
                ensure!(
                    after == Snapshot::read(&mut pruned)?,
                    "prune changed repaired state"
                );
                verify_active(&pruned, keep)?;
                drop(pruned);
                drop(rel);
                fs::remove_file(output)?;
                fs::rename(&scratch, output)?;
                Ok(())
            })();
            if prune_result.is_err() {
                let _ = fs::remove_file(&scratch);
            }
            prune_result?;
        }
        if reclaim {
            let scratch = output.with_extension("repair-reclaim.rel");
            ensure!(!scratch.exists(), "reclaim scratch path already exists");
            let reclaim_result = (|| -> Result<()> {
                let mut rel = Cva::open(output)?;
                rel.reclaim_storage(&scratch)?;
                let mut compact = Cva::open(&scratch)?;
                ensure!(
                    after == Snapshot::read(&mut compact)?,
                    "reclaim changed repaired state"
                );
                verify_active(&compact, keep)?;
                drop(compact);
                drop(rel);
                fs::remove_file(output)?;
                fs::rename(&scratch, output)?;
                Ok(())
            })();
            if reclaim_result.is_err() {
                let _ = fs::remove_file(&scratch);
            }
            reclaim_result?;
        }
        println!(
            "{}",
            serde_json::json!({
                "source":source, "output":output, "keep_profile":crate::util::hex32(&keep.0),
                "retired_profiles":retired, "legacy_import_migrated":migrated,
                "pruned_retired":prune_retired, "reclaimed":reclaim, "verified":true,
                "source_bytes":fs::metadata(source)?.len(), "output_bytes":fs::metadata(output)?.len(),
            })
        );
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(output);
    }
    result
}

fn verify_active(rel: &Cva, keep: CompatibilityProfileId) -> Result<()> {
    let active: BTreeSet<_> = rel
        .active_archive_compatibility_profiles()
        .into_iter()
        .chain(rel.active_memory_compatibility_profiles())
        .map(|p| p.id.0)
        .collect();
    ensure!(
        active == BTreeSet::from([keep.0]),
        "repair did not produce exactly one operational profile"
    );
    Ok(())
}

#[derive(Debug, PartialEq)]
struct Snapshot {
    owner: Option<String>,
    clocks: [u64; 7],
    profiles: BTreeSet<[u8; 32]>,
    archive: String,
    memory: String,
    graph: String,
    freshness: String,
}
impl Snapshot {
    fn read(rel: &mut Cva) -> Result<Self> {
        let mut memory = Vec::new();
        for id in rel.memory_ids() {
            memory.push(rel.memory(id)?);
        }
        Ok(Self {
            owner: rel.owner_id(),
            clocks: [
                rel.latest_global_version(),
                rel.archive_version(),
                rel.memory_version(),
                rel.entity_version(),
                rel.graph_version(),
                rel.relationship_version(),
                rel.vector_version(),
            ],
            profiles: rel
                .compatibility_profiles()
                .into_iter()
                .map(|p| p.id.0)
                .collect(),
            archive: format!(
                "{:?}",
                rel.record_versions()
                    .iter()
                    .map(|v| (v.global_version, v.archive_version))
                    .collect::<Vec<_>>()
            ),
            memory: format!("{:?}", memory),
            graph: format!("{:?}", rel.graph_stats()),
            freshness: format!("{:?}", rel.freshness_records_at(rel.rel_turn_count())?),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use reliquary_memory::{FragmentConfig, SimulatedEmbeddingEndpoint, VectorNormalization};
    #[test]
    fn repair_creates_verified_copy_and_never_overwrites_source_or_existing_output() {
        let root = std::env::current_dir()
            .unwrap()
            .join(".test-tmp")
            .join(format!("repair-{}", uuid_suffix()));
        fs::create_dir_all(&root).unwrap();
        let source = root.join("source.rel");
        let output = root.join("repaired.rel");
        let mut rel = Cva::create(&source).unwrap();
        rel.append_node(
            "n".into(),
            "c".into(),
            None,
            "user".into(),
            1,
            "retained source",
        )
        .unwrap();
        rel.materialize_path_fragments("c", "n", FragmentConfig::default(), true)
            .unwrap();
        let endpoint = SimulatedEmbeddingEndpoint::new(1024, VectorNormalization::L2, 1);
        let canonical = rel.establish_compatibility_profile(&endpoint).unwrap();
        rel.build_archive_vector_generation(canonical.id, &endpoint)
            .unwrap();
        let redundant_endpoint =
            SimulatedEmbeddingEndpoint::new(1024, VectorNormalization::L2, 999);
        let redundant = rel
            .establish_compatibility_profile(&redundant_endpoint)
            .unwrap();
        assert_ne!(canonical.id, redundant.id);
        rel.build_archive_vector_generation(redundant.id, &redundant_endpoint)
            .unwrap();
        rel.sync().unwrap();
        drop(rel);
        let original = fs::read(&source).unwrap();
        run(
            &source,
            &output,
            &crate::util::hex32(&canonical.id.0),
            false,
            false,
        )
        .unwrap();
        assert_eq!(fs::read(&source).unwrap(), original);
        let result = Cva::open(&output).unwrap();
        assert!(!result.compatibility_profile_is_retired(canonical.id));
        assert!(result.compatibility_profile_is_retired(redundant.id));
        drop(result);
        let accepted = fs::read(&output).unwrap();
        assert!(
            run(
                &source,
                &output,
                &crate::util::hex32(&canonical.id.0),
                false,
                false
            )
            .is_err()
        );
        assert_eq!(fs::read(&output).unwrap(), accepted);
        let pruned_output = root.join("pruned.rel");
        run(
            &source,
            &pruned_output,
            &crate::util::hex32(&canonical.id.0),
            true,
            true,
        )
        .unwrap();
        assert_eq!(fs::read(&source).unwrap(), original);
        let mut pruned = Cva::open(&pruned_output).unwrap();
        verify_active(&pruned, canonical.id).unwrap();
        assert!(pruned.compatibility_profile_is_retired(redundant.id));
        assert!(Snapshot::read(&mut pruned).is_ok());
        drop(pruned);
        fs::remove_dir_all(root).unwrap();
    }
    fn uuid_suffix() -> u128 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    }
}
