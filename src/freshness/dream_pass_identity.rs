//! Freshness-owned identity for initial Dream pass events.
//! Dream producers and import attribution share this stable event namespace.
use crate::MemoryId;
pub(crate) fn initial_pass_id(owner: [u8; 16], source: MemoryId) -> String {
    format!(
        "dream-initial:{}:{}",
        owner.iter().map(|b| format!("{b:02x}")).collect::<String>(),
        source
            .0
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>()
    )
}
