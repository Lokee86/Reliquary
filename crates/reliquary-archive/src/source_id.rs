/// Opaque stable target identity carried by Archive source links.
/// Memory owns target existence and lifecycle validation.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct MemoryId(pub [u8; 32]);
