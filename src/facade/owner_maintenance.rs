//! Cross-owner validation of maintenance evidence.
pub(crate) use reliquary_maintenance_store::*;
#[path = "history_prune_manifest.rs"]
mod history;
pub(crate) use history::owner_history_digest;

pub(crate) fn validate_source_history(
    store: &OwnerMaintenanceStore,
    container: &mut reliquary_container::Container,
    graph: &crate::graph_owner::GraphOwner,
) -> Result<(), String> {
    for manifest in store.manifests().values() {
        if Some(manifest.owner_uuid) != container.owner_uuid()
            || manifest.source_global_version > container.latest_version()
            || owner_history_digest(container, manifest.source_global_version, graph)?
                != manifest.source_history_digest
        {
            return Err("maintenance proof does not match its source owner history".into());
        }
    }
    Ok(())
}
