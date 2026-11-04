use super::*;
struct Stub {
    bad: bool,
}
impl EmbeddingEndpoint for Stub {
    fn dimensions(&self) -> u32 {
        2
    }
    fn normalization(&self) -> VectorNormalization {
        VectorNormalization::L2
    }
    fn embed(
        &self,
        mode: EmbeddingMode,
        inputs: &[String],
    ) -> Result<Vec<Vec<f32>>, EmbeddingEndpointError> {
        if self.bad {
            return Ok(vec![vec![f32::NAN, 0.0]; inputs.len()]);
        }
        let base = if mode == EmbeddingMode::Query {
            vec![1.0, 0.0]
        } else {
            vec![0.0, 1.0]
        };
        Ok(vec![base; inputs.len()])
    }
}
#[test]
fn profile_uses_fixed_probe_order_without_store_mutation() {
    let profile = profile_from_endpoint(&Stub { bad: false }).unwrap();
    assert_eq!(
        profile
            .references
            .iter()
            .map(|r| r.mode)
            .collect::<Vec<_>>(),
        [
            EmbeddingMode::Query,
            EmbeddingMode::Query,
            EmbeddingMode::Document,
            EmbeddingMode::Document
        ]
    );
    assert_eq!(profile.id, compatibility_profile_id(&profile));
}
#[test]
fn malformed_endpoint_vectors_rejected() {
    assert!(matches!(
        profile_from_endpoint(&Stub { bad: true }),
        Err(ProbeError::InvalidProfile("invalid embedding vector"))
    ));
}
#[test]
fn profile_compatibility_uses_cosine_floor() {
    let profile = profile_from_endpoint(&Stub { bad: false }).unwrap();
    let report = compare_profiles(&profile, &profile);
    assert!(report.compatible);
    assert_eq!(report.minimum_cosine, Some(1.0));
}
#[test]
fn bounded_contract_defaults_to_unsupported() {
    let error = Stub { bad: false }
        .embed_bounded(EmbeddingMode::Query, &["q".into()], 8)
        .unwrap_err();
    assert!(matches!(
        error,
        EmbeddingEndpointError::InvalidConfiguration("bounded embedding unsupported")
    ));
}

struct BoundedStub;
impl EmbeddingEndpoint for BoundedStub {
    fn supports_bounded_embedding(&self) -> bool {
        true
    }
    fn embed_bounded(
        &self,
        _mode: EmbeddingMode,
        inputs: &[String],
        limit: usize,
    ) -> Result<Vec<Vec<f32>>, EmbeddingEndpointError> {
        let size = inputs.len() * 2 * 4;
        if size > limit {
            return Err(EmbeddingEndpointError::InvalidResponse(
                "bounded output exceeded",
            ));
        }
        Ok(vec![vec![1.0, 0.0]; inputs.len()])
    }
    fn dimensions(&self) -> u32 {
        2
    }
    fn normalization(&self) -> VectorNormalization {
        VectorNormalization::L2
    }
    fn embed(
        &self,
        mode: EmbeddingMode,
        inputs: &[String],
    ) -> Result<Vec<Vec<f32>>, EmbeddingEndpointError> {
        Stub { bad: false }.embed(mode, inputs)
    }
}

#[test]
fn bounded_batch_enforces_bytes_and_validates_response() {
    let input = vec!["a".to_owned(), "b".to_owned()];
    assert!(embed_batch_bounded(&BoundedStub, EmbeddingMode::Query, &input, 16).is_ok());
    assert!(matches!(
        embed_batch_bounded(&BoundedStub, EmbeddingMode::Query, &input, 15),
        Err(ProbeError::Endpoint(
            EmbeddingEndpointError::InvalidResponse("bounded output exceeded")
        ))
    ));
}

struct ProviderStub;
impl reliquary_switchboard::EmbeddingEndpoint for ProviderStub {
    fn dimensions(&self) -> u32 {
        2
    }
    fn normalization(&self) -> reliquary_switchboard::EmbeddingNormalization {
        reliquary_switchboard::EmbeddingNormalization::L2
    }
    fn supports_bounded_embedding(&self) -> bool {
        true
    }
    fn embed_bounded(
        &self,
        mode: reliquary_switchboard::EmbeddingMode,
        inputs: &[String],
        limit: usize,
    ) -> Result<Vec<Vec<f32>>, reliquary_switchboard::EmbeddingEndpointError> {
        if limit < inputs.len() * 8 {
            return Err(
                reliquary_switchboard::EmbeddingEndpointError::InvalidResponse(
                    "bounded output exceeded",
                ),
            );
        }
        let v = match mode {
            reliquary_switchboard::EmbeddingMode::Query => vec![1.0, 0.0],
            reliquary_switchboard::EmbeddingMode::Document => vec![0.0, 1.0],
        };
        Ok(vec![v; inputs.len()])
    }
    fn embed(
        &self,
        mode: reliquary_switchboard::EmbeddingMode,
        inputs: &[String],
    ) -> Result<Vec<Vec<f32>>, reliquary_switchboard::EmbeddingEndpointError> {
        self.embed_bounded(mode, inputs, usize::MAX)
    }
}

#[test]
fn switchboard_provider_adapter_preserves_mode_bounds_and_errors() {
    let endpoint = SwitchboardEmbeddingEndpoint::new(std::sync::Arc::new(ProviderStub));
    assert!(embed_batch_bounded(&endpoint, EmbeddingMode::Document, &["doc".into()], 8).is_ok());
    assert!(matches!(
        embed_batch_bounded(&endpoint, EmbeddingMode::Query, &["query".into()], 7),
        Err(ProbeError::Endpoint(
            EmbeddingEndpointError::InvalidResponse("bounded output exceeded")
        ))
    ));
}
