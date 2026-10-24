pub use crate::maintenance::attachment_repack::ProjectAttachmentRepackResult;
use crate::{Cva, CvaError};
use std::path::Path;

impl Cva {
    pub fn repack_project_backed_attachments(
        &mut self,
        output_path: impl AsRef<Path>,
    ) -> Result<ProjectAttachmentRepackResult, CvaError> {
        self.owner.repack_project_backed_attachments(output_path)
    }
}

#[cfg(test)]
use crate::ObjectRef;
#[cfg(test)]
use std::collections::{BTreeMap, HashSet};
#[cfg(test)]
use std::fs;

#[cfg(test)]
mod tests {
    use super::*;

    fn object(offset: u64, len: u64) -> ObjectRef {
        let mut bytes = [0_u8; 16];
        bytes[..8].copy_from_slice(&offset.to_le_bytes());
        bytes[8..].copy_from_slice(&len.to_le_bytes());
        ObjectRef::from_legacy_bytes(bytes)
    }

    #[test]
    fn relocate_payload_rewrites_current_vector_generation_object_ref() {
        let old = object(100, 24);
        let new = object(80, 24);
        let relocated = BTreeMap::from([(old, new)]);

        let generation = crate::vector_generation_codec::GenerationVersion {
            global_version: 3,
            vector_version: 2,
            record: old,
        };
        let generation = crate::owner_lifecycle::relocation::relocate_payload(
            &crate::vector_generation_codec::encode_version(generation),
            &relocated,
        )
        .unwrap();
        assert_eq!(
            crate::vector_generation_codec::decode_version(&generation)
                .unwrap()
                .unwrap()
                .record,
            new
        );
    }
}
