pub(crate) fn stale_snapshot() -> crate::EntityResolverError {
    crate::EntityResolverError::InvalidOutput("Entity resolution snapshot became stale".into())
}
