use crate::rel_metadata_codec::{decode_rel_metadata, encode_rel_metadata};
use crate::{Container, RelMetadata};

#[derive(Default)]
pub(crate) struct RelMetadataStore {
    current: Option<RelMetadata>,
}

impl RelMetadataStore {
    pub(crate) fn ingest(&mut self, payload: &[u8]) -> Result<(), String> {
        if let Some(metadata) = decode_rel_metadata(payload)? {
            self.current = Some(metadata);
        }
        Ok(())
    }

    pub(crate) fn effective(&self, container: &Container) -> RelMetadata {
        if let Some(metadata) = self.current() {
            return metadata.clone();
        }
        crate::RelMetadata {
            type_label: container
                .identity()
                .and_then(|identity| identity.scope)
                .map(|scope| match scope {
                    crate::ReliquaryScopeKind::Organization => "Organization".to_owned(),
                    crate::ReliquaryScopeKind::Project => "Project".to_owned(),
                    crate::ReliquaryScopeKind::Connection => "Connection".to_owned(),
                }),
            dependencies: Vec::new(),
        }
    }

    pub(crate) fn current(&self) -> Option<&RelMetadata> {
        self.current.as_ref()
    }

    pub(crate) fn put(
        &mut self,
        container: &mut Container,
        metadata: RelMetadata,
    ) -> Result<bool, String> {
        if self.current.as_ref() == Some(&metadata) {
            return Ok(false);
        }
        let payload = encode_rel_metadata(&metadata)?;
        container
            .append(&payload)
            .map_err(|error| error.to_string())?;
        self.current = Some(metadata);
        Ok(true)
    }
}
