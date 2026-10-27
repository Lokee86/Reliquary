pub(crate) use reliquary_external_index_store::store::*;

pub(crate) trait SourceStatus {
    fn status(&self) -> super::model::ExternalIndexStatus;
}
impl SourceStatus for SourceState {
    fn status(&self) -> super::model::ExternalIndexStatus {
        super::model::ExternalIndexStatus {
            source_namespace: self.namespace.clone(),
            generation: self.generation,
            revision: self.revision,
            cursor: self.cursor,
            messages: self.messages.len(),
            chunks: self.messages.iter().map(|m| m.chunks.len()).sum(),
            compatibility_profile: crate::CompatibilityProfileId(self.profile),
        }
    }
}
