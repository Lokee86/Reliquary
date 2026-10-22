//! Validate disposable external-index snapshots against their durable owners.

pub(crate) fn validate_external_index_backing(
    index: &mut crate::external_index::store::ExternalIndexStore,
    packed: &crate::packed_vector_store::PackedVectorStore,
    profiles: &crate::compatibility_profile_store::CompatibilityProfileStore,
) {
    let invalid = index
        .source_states()
        .filter(|s| {
            !(profiles
                .get(crate::CompatibilityProfileId(s.profile))
                .is_some_and(|p| p.dimensions == s.dimensions)
                && s.messages.iter().flat_map(|m| &m.chunks).all(|c| {
                    packed
                        .info(crate::PackedVectorId(c.matrix))
                        .is_some_and(|i| {
                            i.schema.dimensions == s.dimensions
                                && i.schema.scalar == crate::ScalarType::F32
                                && u64::from(c.row) < i.count
                        })
                }))
        })
        .map(|s| s.namespace.clone())
        .collect();
    index.discard_invalid_backing(&invalid);
}
