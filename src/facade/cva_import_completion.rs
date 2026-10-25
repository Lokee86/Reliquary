//! Thin Cva adapter for native Import completion.
use crate::imports::ImportCompletionOutcome;
use crate::{Cva, CvaError};

impl Cva {
    /// Exclusive offline completion. Import owns recovery, refusal and
    /// publication ordering; this adapter supplies exact owner views.
    pub fn complete_native_import(
        &mut self,
        import_id: &str,
        now_ns: i64,
    ) -> Result<ImportCompletionOutcome, CvaError> {
        crate::imports::owner_completion::complete(&mut self.owner, import_id, now_ns)
    }
}
