use crate::{
    ConfiguredGeneralEndpoint, EmbeddingEndpoint, EmbeddingMode, InsomniaExtractor,
    ModelSwitchboard, OpenAiReadyEmbeddingEndpoint, VectorNormalization,
};

pub(super) fn prepare_insomnia(
    switchboard: &ModelSwitchboard,
    embedding_batch_size: usize,
    embedding_concurrency: usize,
    has_phy: bool,
) -> Result<
    (
        InsomniaExtractor<ConfiguredGeneralEndpoint>,
        OpenAiReadyEmbeddingEndpoint,
    ),
    String,
> {
    let main = ConfiguredGeneralEndpoint::from_insomnia_switchboard(switchboard)
        .map_err(|error| error.to_string())?;
    let embedding = embedding_endpoint(switchboard, embedding_batch_size, embedding_concurrency)?;
    let chronos = ConfiguredGeneralEndpoint::from_chronos_switchboard(switchboard)
        .map_err(|error| error.to_string())?;
    let mut extractor = InsomniaExtractor::new(main).with_temporal_endpoint(chronos);
    if switchboard.insomnia_metadata().is_some() {
        extractor = extractor.with_metadata_endpoint(
            ConfiguredGeneralEndpoint::from_insomnia_metadata_switchboard(switchboard)
                .map_err(|error| error.to_string())?,
        );
    }
    if switchboard.entity_extraction().is_some() {
        extractor = extractor.with_enrichment_endpoint(
            ConfiguredGeneralEndpoint::from_entity_extraction_switchboard(switchboard)
                .map_err(|error| error.to_string())?,
        );
    }
    if has_phy {
        extractor = extractor.with_ownership_endpoint(
            ConfiguredGeneralEndpoint::from_insomnia_ownership_switchboard(switchboard)
                .map_err(|error| error.to_string())?,
        );
    }
    Ok((extractor, embedding))
}

pub(super) fn embedding_endpoint(
    switchboard: &ModelSwitchboard,
    batch_size: usize,
    concurrency: usize,
) -> Result<OpenAiReadyEmbeddingEndpoint, String> {
    OpenAiReadyEmbeddingEndpoint::from_switchboard(switchboard)
        .map_err(|error| error.to_string())?
        .with_batching(batch_size, concurrency)
        .map_err(|error| error.to_string())
}

pub(super) fn probe_embedding(
    switchboard: &ModelSwitchboard,
    text: &str,
) -> Result<(usize, usize, VectorNormalization), String> {
    let endpoint = OpenAiReadyEmbeddingEndpoint::from_switchboard(switchboard)
        .map_err(|error| error.to_string())?;
    let vectors = endpoint
        .embed(EmbeddingMode::Query, &[text.to_owned()])
        .map_err(|error| error.to_string())?;
    Ok((
        vectors.len(),
        vectors.first().map(Vec::len).unwrap_or(0),
        endpoint.normalization(),
    ))
}
