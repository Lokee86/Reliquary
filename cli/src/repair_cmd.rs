//! Copy-only REL/PHY repair. Source artifacts are never opened for mutation.
use anyhow::{Context, Result, ensure};
use reliquary_memory::owner_lifecycle::LifecycleTarget;
use reliquary_memory::{CompatibilityProfileId, Cva, HistoryPruneRequest, Phylactery};
use std::{
    collections::BTreeSet,
    fs::{self, File, OpenOptions},
    path::Path,
};

pub fn run_rel(
    source: &Path,
    output: &Path,
    keep: &[String],
    prune_retired: bool,
    reclaim: bool,
) -> Result<()> {
    let keep = parse_profiles(keep)?;
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
        for profile in &keep {
            rel.compatibility_profile(*profile)
                .context("requested retained profile is absent")?;
            ensure!(
                !rel.compatibility_profile_is_retired(*profile),
                "requested retained profile was already retired"
            );
            ensure!(
                rel.current_vector_generation(*profile).is_some(),
                "requested profile Archive generation is not complete/current"
            );
            if rel.memory_stats().memories != 0 {
                ensure!(
                    rel.active_memory_compatibility_profile(*profile).is_some(),
                    "requested profile Memory vectors do not cover current bodies"
                );
            }
        }
        let migrated = rel.normalize_import_settlements()?;
        let mut retired = Vec::new();
        for profile in rel.compatibility_profiles() {
            if !keep.contains(&profile.id) && rel.retire_compatibility_profile(profile.id)? {
                retired.push(crate::util::hex32(&profile.id.0));
            }
        }
        rel.sync()?;
        let after = Snapshot::read(&mut rel)?;
        ensure!(
            before == after,
            "repair changed semantic/version/profile identity state"
        );
        verify_active(&rel, &keep)?;
        drop(rel);
        let mut reopened = Cva::open(output)?;
        ensure!(
            after == Snapshot::read(&mut reopened)?,
            "reopen changed repaired state"
        );
        verify_active(&reopened, &keep)?;
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
                verify_active(&pruned, &keep)?;
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
                verify_active(&compact, &keep)?;
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
                "source":source, "output":output, "keep_profiles":keep.iter().map(|id| crate::util::hex32(&id.0)).collect::<Vec<_>>(),
                "retired_profiles":retired, "import_settlements_normalized":migrated,
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

pub fn run_phy(
    source: &Path,
    output: &Path,
    keep_values: &[String],
    prune_retired: bool,
    reclaim: bool,
) -> Result<()> {
    let keep = parse_profiles(keep_values)?;
    ensure!(!output.exists(), "repair output already exists");
    let mut input = File::open(source).context("open retained PHY source")?;
    let mut copied = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(output)
        .context("create disposable PHY repair copy")?;
    let result = (|| -> Result<()> {
        std::io::copy(&mut input, &mut copied)?;
        copied.sync_all()?;
        drop(copied);
        let mut phy = Phylactery::open(output)?;
        let before = PhySnapshot::read(&mut phy)?;
        for profile in &keep {
            phy.compatibility_profile(*profile)
                .context("requested retained profile is absent")?;
            ensure!(
                !phy.compatibility_profile_is_retired(*profile),
                "requested retained profile was already retired"
            );
            if phy.memory_stats().memories != 0 {
                ensure!(
                    phy.active_memory_compatibility_profile(*profile).is_some(),
                    "requested profile Memory vectors do not cover current bodies"
                );
            }
        }
        let mut retired = Vec::new();
        for profile in phy.compatibility_profiles() {
            if !keep.contains(&profile.id) && phy.retire_compatibility_profile(profile.id)? {
                retired.push(crate::util::hex32(&profile.id.0));
            }
        }
        phy.sync()?;
        let after = PhySnapshot::read(&mut phy)?;
        ensure!(
            before == after,
            "repair changed semantic/version/profile identity state"
        );
        verify_phy_active(&phy, &keep)?;
        drop(phy);
        let mut reopened = Phylactery::open(output)?;
        ensure!(
            after == PhySnapshot::read(&mut reopened)?,
            "reopen changed repaired PHY state"
        );
        verify_phy_active(&reopened, &keep)?;
        drop(reopened);

        if prune_retired {
            let scratch = output.with_extension("repair-prune.phy");
            ensure!(!scratch.exists(), "prune scratch path already exists");
            let prune_result = (|| -> Result<()> {
                let mut phy = Phylactery::open(output)?;
                let targets = phy
                    .compatibility_profiles()
                    .into_iter()
                    .filter(|profile| phy.compatibility_profile_is_retired(profile.id))
                    .map(|profile| LifecycleTarget::CompatibilityProfile(profile.id))
                    .collect::<Vec<_>>();
                ensure!(
                    !targets.is_empty(),
                    "no retired profiles selected for pruning"
                );
                phy.prune_tombstoned_history(&scratch, HistoryPruneRequest { targets })?;
                let mut pruned = Phylactery::open(&scratch)?;
                ensure!(
                    after == PhySnapshot::read(&mut pruned)?,
                    "prune changed repaired PHY state"
                );
                verify_phy_active(&pruned, &keep)?;
                drop(pruned);
                drop(phy);
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
            let scratch = output.with_extension("repair-reclaim.phy");
            ensure!(!scratch.exists(), "reclaim scratch path already exists");
            let reclaim_result = (|| -> Result<()> {
                let mut phy = Phylactery::open(output)?;
                phy.reclaim_storage(&scratch)?;
                let mut compact = Phylactery::open(&scratch)?;
                ensure!(
                    after == PhySnapshot::read(&mut compact)?,
                    "reclaim changed repaired PHY state"
                );
                verify_phy_active(&compact, &keep)?;
                drop(compact);
                drop(phy);
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
                "source":source, "output":output,
                "keep_profiles":keep.iter().map(|id| crate::util::hex32(&id.0)).collect::<Vec<_>>(),
                "retired_profiles":retired, "pruned_retired":prune_retired, "reclaimed":reclaim,
                "verified":true, "source_bytes":fs::metadata(source)?.len(), "output_bytes":fs::metadata(output)?.len(),
            })
        );
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(output);
    }
    result
}

fn verify_phy_active(phy: &Phylactery, keep: &[CompatibilityProfileId]) -> Result<()> {
    let active: BTreeSet<_> = phy
        .active_memory_compatibility_profiles()
        .into_iter()
        .map(|p| p.id.0)
        .collect();
    ensure!(
        active == keep.iter().map(|profile| profile.0).collect(),
        "PHY repair did not produce exactly the requested operational profiles"
    );
    Ok(())
}

#[derive(Debug, PartialEq)]
struct PhySnapshot {
    owner: Option<String>,
    clocks: [u64; 4],
    profiles: BTreeSet<[u8; 32]>,
    memories: String,
    graph: String,
}
impl PhySnapshot {
    fn read(phy: &mut Phylactery) -> Result<Self> {
        let memories = phy
            .memory_ids()
            .into_iter()
            .map(|id| phy.memory(id))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self {
            owner: phy.owner_id(),
            clocks: [
                phy.latest_global_version(),
                phy.memory_version(),
                phy.entity_version(),
                phy.relationship_version(),
            ],
            profiles: phy
                .compatibility_profiles()
                .into_iter()
                .map(|p| p.id.0)
                .collect(),
            memories: format!("{memories:?}"),
            graph: format!("{:?}:{:?}", phy.graph_stats(), phy.relationship_stats()),
        })
    }
}

fn parse_profiles(values: &[String]) -> Result<Vec<CompatibilityProfileId>> {
    ensure!(
        !values.is_empty(),
        "at least one --keep-profile is required"
    );
    let profiles = values
        .iter()
        .map(|value| crate::util::parse_profile_id(value))
        .collect::<Result<Vec<_>>>()?;
    ensure!(
        profiles
            .iter()
            .map(|profile| profile.0)
            .collect::<BTreeSet<_>>()
            .len()
            == profiles.len(),
        "duplicate --keep-profile value"
    );
    Ok(profiles)
}

fn verify_active(rel: &Cva, keep: &[CompatibilityProfileId]) -> Result<()> {
    let active: BTreeSet<_> = rel
        .active_archive_compatibility_profiles()
        .into_iter()
        .chain(rel.active_memory_compatibility_profiles())
        .map(|p| p.id.0)
        .collect();
    ensure!(
        active == keep.iter().map(|profile| profile.0).collect(),
        "repair did not produce exactly the requested operational profiles"
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
    use reliquary_memory::{
        FragmentConfig, MemoryDraft, SimulatedEmbeddingEndpoint, VectorNormalization,
    };
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
        run_rel(
            &source,
            &output,
            &[crate::util::hex32(&canonical.id.0)],
            false,
            false,
        )
        .unwrap();
        assert_eq!(fs::read(&source).unwrap(), original);
        let result = Cva::open(&output).unwrap();
        assert!(!result.compatibility_profile_is_retired(canonical.id));
        assert!(result.compatibility_profile_is_retired(redundant.id));
        drop(result);
        let multi_output = root.join("multi-profile.rel");
        run_rel(
            &source,
            &multi_output,
            &[
                crate::util::hex32(&canonical.id.0),
                crate::util::hex32(&redundant.id.0),
            ],
            false,
            false,
        )
        .unwrap();
        let multi = Cva::open(&multi_output).unwrap();
        verify_active(&multi, &[canonical.id, redundant.id]).unwrap();
        assert!(!multi.compatibility_profile_is_retired(redundant.id));
        drop(multi);
        let accepted = fs::read(&output).unwrap();
        assert!(
            run_rel(
                &source,
                &output,
                &[crate::util::hex32(&canonical.id.0)],
                false,
                false
            )
            .is_err()
        );
        assert_eq!(fs::read(&output).unwrap(), accepted);
        let pruned_output = root.join("pruned.rel");
        run_rel(
            &source,
            &pruned_output,
            &[crate::util::hex32(&canonical.id.0)],
            true,
            true,
        )
        .unwrap();
        assert_eq!(fs::read(&source).unwrap(), original);
        let mut pruned = Cva::open(&pruned_output).unwrap();
        verify_active(&pruned, &[canonical.id]).unwrap();
        assert!(pruned.compatibility_profile_is_retired(redundant.id));
        assert!(Snapshot::read(&mut pruned).is_ok());
        drop(pruned);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn phy_repair_retains_requested_profile_set_in_a_verified_copy() {
        let root = std::env::current_dir()
            .unwrap()
            .join(".test-tmp")
            .join(format!("phy-repair-{}", uuid_suffix()));
        fs::create_dir_all(&root).unwrap();
        let source = root.join("source.phy");
        let output = root.join("repaired.phy");
        let mut phy = Phylactery::create(&source).unwrap();
        phy.publish_memory(None, 0, memory_draft("m1", "retained", "retained body"))
            .unwrap();
        let first_endpoint = SimulatedEmbeddingEndpoint::new(8, VectorNormalization::L2, 11);
        let first = phy
            .establish_compatibility_profile(&first_endpoint)
            .unwrap();
        phy.build_missing_memory_vectors(first.id, &first_endpoint)
            .unwrap();
        let second_endpoint = SimulatedEmbeddingEndpoint::new(8, VectorNormalization::L2, 12);
        let second = phy
            .establish_compatibility_profile(&second_endpoint)
            .unwrap();
        phy.build_missing_memory_vectors(second.id, &second_endpoint)
            .unwrap();
        let third_endpoint = SimulatedEmbeddingEndpoint::new(8, VectorNormalization::L2, 13);
        let third = phy
            .establish_compatibility_profile(&third_endpoint)
            .unwrap();
        phy.build_missing_memory_vectors(third.id, &third_endpoint)
            .unwrap();
        phy.sync().unwrap();
        drop(phy);
        let original = fs::read(&source).unwrap();

        run_phy(
            &source,
            &output,
            &[
                crate::util::hex32(&first.id.0),
                crate::util::hex32(&second.id.0),
            ],
            false,
            false,
        )
        .unwrap();
        assert_eq!(fs::read(&source).unwrap(), original);
        let repaired = Phylactery::open(&output).unwrap();
        verify_phy_active(&repaired, &[first.id, second.id]).unwrap();
        assert!(repaired.compatibility_profile_is_retired(third.id));
        drop(repaired);

        let pruned_output = root.join("pruned.phy");
        run_phy(
            &source,
            &pruned_output,
            &[crate::util::hex32(&first.id.0)],
            true,
            true,
        )
        .unwrap();
        assert_eq!(fs::read(&source).unwrap(), original);
        let pruned = Phylactery::open(&pruned_output).unwrap();
        verify_phy_active(&pruned, &[first.id]).unwrap();
        assert!(pruned.compatibility_profile_is_retired(second.id));
        assert!(pruned.compatibility_profile_is_retired(third.id));
        drop(pruned);
        fs::remove_dir_all(root).unwrap();
    }

    fn memory_draft(mutation_id: &str, title: &str, content: &str) -> MemoryDraft {
        MemoryDraft {
            category: "preference".into(),
            memory_type: "user".into(),
            authority_kind: "direct".into(),
            temporal_status: "unknown".into(),
            title: title.into(),
            content: content.into(),
            scope: "user".into(),
            lifecycle_state: "extracted".into(),
            archived: false,
            superseded_by: None,
            parent_id: None,
            source_node_id: None,
            content_source_conversation_id: None,
            content_source_node_id: None,
            grounding_source_conversation_id: None,
            grounding_source_node_id: None,
            source_episode_id: None,
            source_time_ns: None,
            mutation_id: mutation_id.into(),
            created_at_ns: 10,
            updated_at_ns: 10,
        }
    }
    fn uuid_suffix() -> u128 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    }
}
