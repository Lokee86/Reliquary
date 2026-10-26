use reliquary_container::ObjectRef;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ArchiveRecordVersion {
    pub global_version: u64,
    pub archive_version: u64,
    pub record: ObjectRef,
}
