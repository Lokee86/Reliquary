use crate::args::RelCommand;
use crate::util::{file_len, hex32};
use anyhow::Result;
use reliquary_memory::Reliquary;

pub fn run(command: RelCommand) -> Result<()> {
    match command {
        RelCommand::Create { path, type_label } => {
            let mut rel = Reliquary::create(&path)?;
            if type_label.is_some() {
                rel.set_rel_metadata(type_label, Vec::new())?;
                rel.sync()?;
            }
            let metadata = rel.rel_metadata();
            println!(
                "created REL: {} id={} type={}",
                path.display(),
                rel.owner_id().as_deref().unwrap_or("none"),
                metadata.type_label.as_deref().unwrap_or("none")
            );
        }
        RelCommand::Info { path } => info(&path)?,
        RelCommand::Repair {
            source,
            output,
            keep_profile,
            reclaim,
        } => crate::repair_cmd::run(&source, &output, &keep_profile, reclaim)?,
        RelCommand::Verify { path } => {
            let rel = Reliquary::open(&path)?;
            let metadata = rel.rel_metadata();
            println!(
                "REL ok: {} id={} type={} dependencies={} legacy_cva={} archive_version={} relationship_version={} vector_version={}",
                path.display(),
                rel.owner_id().as_deref().unwrap_or("none"),
                metadata.type_label.as_deref().unwrap_or("none"),
                metadata.dependencies.len(),
                rel.is_legacy_cva(),
                rel.archive_version(),
                rel.relationship_version(),
                rel.vector_version()
            );
        }
        RelCommand::Reclaim { path, output } => {
            let mut rel = Reliquary::open(&path)?;
            let report = rel.reclaim_storage(&output)?;
            println!(
                "REL reclaimed: {} -> {} chunks={} bytes={} source_bytes={} output_bytes={}",
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
                "reclaimed: unpublished_backing={} orphan_content={} orphan_memory_bodies={} unactivated_archive_vectors={} unreferenced_packed_vectors={} free_compaction={}",
                report.unpublished_backing_chunks,
                report.orphan_content_chunks,
                report.orphan_memory_body_chunks,
                report.unactivated_archive_vector_chunks,
                report.unreferenced_packed_vector_chunks,
                report.free_compaction_chunks
            );
        }
    }
    Ok(())
}

fn info(path: &std::path::Path) -> Result<()> {
    let rel = Reliquary::open(path)?;
    let archive = rel.stats();
    let memories = rel.memory_stats();
    let relationships = rel.relationship_stats();
    let graph = rel.graph_stats();
    let packed = rel.packed_vector_stats();
    let memory_vectors = rel.memory_vector_stats();
    let bindings = rel.archive_vector_stats();
    let profiles = rel.compatibility_profile_stats();
    let generations = rel.vector_generation_stats();
    println!("path: {}", path.display());
    println!("bytes: {}", file_len(path)?);
    println!("kind: reliquary");
    println!("id: {}", rel.owner_id().as_deref().unwrap_or("none"));
    let metadata = rel.rel_metadata();
    println!("type: {}", metadata.type_label.as_deref().unwrap_or("none"));
    println!("dependencies: {}", metadata.dependencies.join(", "));
    println!("legacy_cva: {}", rel.is_legacy_cva());
    println!("archive_version: {}", rel.archive_version());
    println!("memory_version: {}", rel.memory_version());
    println!("relationship_version: {}", rel.relationship_version());
    println!("graph_version: {}", rel.graph_version());
    println!("vector_version: {}", rel.vector_version());
    println!(
        "archive: nodes={} branches={} fragments={} files={} source_attachments={} file_memory_links={} content_objects={}",
        archive.nodes,
        archive.branches,
        archive.fragments,
        archive.files,
        archive.source_attachments,
        archive.file_memory_links,
        archive.content_objects
    );
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
    println!("memory_vectors: bindings={}", memory_vectors.bindings);
    println!(
        "archive_vectors: objects={} rows={}",
        bindings.objects, bindings.rows
    );
    println!("compatibility_profiles: {}", profiles.profiles);
    println!(
        "vector_generations: total={} active_profiles={}",
        generations.generations, generations.active_profiles
    );
    for profile in rel.compatibility_profiles() {
        if let Some(generation) = rel.current_vector_generation(profile.id) {
            println!(
                "active: profile={} generation={} source_archive_version={} vector_version={}",
                hex32(&profile.id.0),
                hex32(&generation.id.0),
                generation.source_archive_version,
                generation.vector_version
            );
        }
    }
    Ok(())
}
