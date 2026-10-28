use reliquary_vectors::{
    EmbeddingEndpoint, EmbeddingEndpointError, EmbeddingMode, VectorNormalization,
};

struct ExternalEndpoint;

impl EmbeddingEndpoint for ExternalEndpoint {
    fn dimensions(&self) -> u32 {
        1
    }
    fn normalization(&self) -> VectorNormalization {
        VectorNormalization::None
    }
    fn embed(
        &self,
        _mode: EmbeddingMode,
        inputs: &[String],
    ) -> Result<Vec<Vec<f32>>, EmbeddingEndpointError> {
        Ok(inputs.iter().map(|_| vec![1.0]).collect())
    }
}

#[test]
fn downstream_crates_can_implement_endpoint() {
    let profile =
        reliquary_vectors::compatibility_profile_probe::profile_from_endpoint(&ExternalEndpoint)
            .unwrap();
    assert_eq!(profile.dimensions, 1);
    assert_eq!(profile.references.len(), 4);
}
