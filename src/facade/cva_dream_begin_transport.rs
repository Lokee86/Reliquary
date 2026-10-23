//! Supported facade adapters for exact owner-local Dream Begin evidence.
use crate::{Cva, CvaError, InitialDreamPass, ObjectRef};
impl Cva {
    pub(crate) fn materialized_dream_begin(
        &mut self,
        object: ObjectRef,
        payload: &[u8],
    ) -> Result<Option<InitialDreamPass>, CvaError> {
        crate::owner_lifecycle::dream_begin_transport::materialized_dream_begin(
            &self.container,
            &self.archive,
            &self.memories,
            object,
            payload,
        )
    }
    pub(crate) fn current_dream_begin_payload(
        &mut self,
        pass: &InitialDreamPass,
    ) -> Result<Vec<u8>, CvaError> {
        crate::owner_lifecycle::dream_begin_transport::current_dream_begin_payload(
            &self.container,
            &self.archive,
            &self.memories,
            pass,
        )
    }
}
