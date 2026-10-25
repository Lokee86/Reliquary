use crate::{
    DreamPairClassification, DreamPairVerification, DreamPublicationError, DreamPublicationOutcome,
    DreamVerificationPolicy, Phylactery,
};

impl Phylactery {
    pub fn publish_dream_pair(
        &mut self,
        classification: &DreamPairClassification,
        verification: Option<&DreamPairVerification>,
        policy: DreamVerificationPolicy,
        expected_graph_version: u64,
    ) -> Result<DreamPublicationOutcome, DreamPublicationError> {
        self.native_publish_dream_pair(classification, verification, policy, expected_graph_version)
    }
}
