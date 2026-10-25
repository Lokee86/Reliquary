//! Supported facade adapters for exact owner-local Dream Begin evidence.
use crate::{Cva, CvaError, InitialDreamPass, ObjectRef};
impl Cva {
    pub(crate) fn materialized_dream_begin(
        &mut self,
        object: ObjectRef,
        payload: &[u8],
    ) -> Result<Option<InitialDreamPass>, CvaError> {
        self.runtime_materialized_dream_begin(object, payload)
    }
    pub(crate) fn current_dream_begin_payload(
        &mut self,
        pass: &InitialDreamPass,
    ) -> Result<Vec<u8>, CvaError> {
        self.runtime_current_dream_begin_payload(pass)
    }
}
