use super::*;

/// Initialize the domain formats belonging to one newly created physical owner.
pub(super) fn initialize_owner(container: &mut Container) -> Result<(), OwnerSessionError> {
    MemoryStore::create(container)?;
    PackedVectorStore::default().initialize(container)?;
    CompatibilityProfileStore::default().initialize(container)?;
    MemoryVectorStore::default().initialize(container)?;
    if container.identity().file_kind == FileKind::Reliquary {
        ArchiveVectorStore::default().initialize(container)?;
        VectorGenerationStore::empty().initialize(container)?;
        FreshnessStore::new(Some(container.owner_id()))
            .initialize(container)
            .map_err(OwnerSessionError::Record)?;
    }
    container.sync()?;
    Ok(())
}
