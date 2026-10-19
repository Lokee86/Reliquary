use crate::args::PhyCommand;
use crate::util::{file_len, hex32};
use anyhow::Result;
use reliquary_memory::Phylactery;

pub fn run(command: PhyCommand) -> Result<()> {
    match command {
        PhyCommand::Create { path } => {
            let phy = Phylactery::create(&path)?;
            println!(
                "created PHY: {} id={}",
                path.display(),
                phy.owner_id().as_deref().unwrap_or("none")
            );
        }
        PhyCommand::Info { path } => info(&path)?,
        PhyCommand::Verify { path } => {
            let phy = Phylactery::open(&path)?;
            println!(
                "PHY ok: {} id={} memory_version={} relationship_version={} graph_version={}",
                path.display(),
                phy.owner_id().as_deref().unwrap_or("none"),
                phy.memory_version(),
                phy.relationship_version(),
                phy.graph_version()
            );
        }
        PhyCommand::Repair {
            source,
            output,
            keep_profile,
            prune_retired,
            reclaim,
        } => {
            crate::repair_cmd::run_phy(&source, &output, &keep_profile, prune_retired, reclaim)?;
        }
        PhyCommand::Rewrite {
            source,
            output,
            dry_run,
            policy_file,
            profiles,
            include_memories,
            exclude_memories,
        } => {
            crate::rewrite_cmd::run_phy(
                &source,
                &output,
                dry_run,
                policy_file.as_deref(),
                &profiles,
                &include_memories,
                &exclude_memories,
            )?;
        }
        PhyCommand::Reclaim { path, output } => {
            let mut phy = Phylactery::open(&path)?;
            let report = phy.reclaim_storage(&output)?;
            println!(
                "PHY reclaimed: {} -> {} chunks={} bytes={} source_bytes={} output_bytes={}",
                path.display(),
                output.display(),
                report.reclaimed_chunks,
                report
                    .source_file_bytes
                    .saturating_sub(report.output_file_bytes),
                report.source_file_bytes,
                report.output_file_bytes
            );
            println!(
                "reclaimed: unpublished_backing={} orphan_content={} orphan_memory_bodies={} unreferenced_packed_vectors={}",
                report.unpublished_backing_chunks,
                report.orphan_content_chunks,
                report.orphan_memory_body_chunks,
                report.unreferenced_packed_vector_chunks
            );
        }
    }
    Ok(())
}

fn info(path: &std::path::Path) -> Result<()> {
    let phy = Phylactery::open(path)?;
    let memories = phy.memory_stats();
    let relationships = phy.relationship_stats();
    let graph = phy.graph_stats();
    let packed = phy.packed_vector_stats();
    let vectors = phy.memory_vector_stats();
    let profiles = phy.compatibility_profile_stats();

    println!("path: {}", path.display());
    println!("bytes: {}", file_len(path)?);
    println!("kind: phylactery");
    println!("id: {}", phy.owner_id().as_deref().unwrap_or("none"));
    println!("memory_version: {}", phy.memory_version());
    println!("relationship_version: {}", phy.relationship_version());
    println!("graph_version: {}", phy.graph_version());
    println!(
        "memories: current={} revisions={} bodies={}",
        memories.memories, memories.revisions, memories.bodies
    );
    println!(
        "relationships: current={} revisions={}",
        relationships.relationships, relationships.revisions
    );
    println!("graph: active_relations={}", graph.active_relations);
    println!(
        "packed_vectors: objects={} rows={} matrix_bytes={}",
        packed.objects, packed.rows, packed.matrix_bytes
    );
    println!("memory_vectors: bindings={}", vectors.bindings);
    println!("compatibility_profiles: {}", profiles.profiles);
    for profile in phy.active_memory_compatibility_profiles() {
        println!(
            "active_memory: profile={} dimensions={}",
            hex32(&profile.id.0),
            profile.dimensions
        );
    }
    Ok(())
}
