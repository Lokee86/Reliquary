//! Physical REL/PHY container framing, clocks, leases, cursors, and explicit repair.
pub mod codec;
mod container;
mod container_error;
mod version;
pub use container::ContainerError;
pub use container::publication_envelope;
pub use container::{
    Container, ContainerIdentity, FileKind, FormatVersion, ObjectRef, PhysicalCursor,
    PhysicalFrame, PhysicalReadSource, ReliquaryScopeKind, ScanClockState, TailRepairPlan,
    WriteAdmission, WriteScope,
};

pub mod review_clocks;
