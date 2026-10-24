//! Test-only conversion for focused native reconciliation helper fixtures.
pub(super) fn into_owner(cva: crate::Cva) -> crate::owner_lifecycle::ReliquaryOwner {
    cva.into_owner()
}
