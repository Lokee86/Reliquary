use crate::{EmbeddingEndpointError, EmbeddingNormalization};
use serde_json::Value;

pub(crate) fn decode_response(
    bytes: &[u8],
    expected: usize,
    dimensions: u32,
    normalization: EmbeddingNormalization,
) -> Result<Vec<Vec<f32>>, EmbeddingEndpointError> {
    let value: Value = serde_json::from_slice(bytes)
        .map_err(|_| EmbeddingEndpointError::InvalidResponse("invalid JSON response"))?;
    let data = value["data"]
        .as_array()
        .ok_or(EmbeddingEndpointError::InvalidResponse(
            "missing embedding data",
        ))?;
    if data.len() != expected {
        return Err(EmbeddingEndpointError::InvalidResponse(
            "embedding count mismatch",
        ));
    }
    let mut output = vec![None; expected];
    for item in data {
        let index = item["index"]
            .as_u64()
            .and_then(|value| usize::try_from(value).ok())
            .filter(|index| *index < expected)
            .ok_or(EmbeddingEndpointError::InvalidResponse(
                "invalid embedding index",
            ))?;
        if output[index].is_some() {
            return Err(EmbeddingEndpointError::InvalidResponse(
                "duplicate embedding index",
            ));
        }
        let raw = item["embedding"]
            .as_array()
            .ok_or(EmbeddingEndpointError::InvalidResponse(
                "missing embedding vector",
            ))?;
        if raw.len() != dimensions as usize {
            return Err(EmbeddingEndpointError::InvalidResponse(
                "embedding dimension mismatch",
            ));
        }
        let mut vector = Vec::with_capacity(raw.len());
        for value in raw {
            let value = value.as_f64().filter(|value| value.is_finite()).ok_or(
                EmbeddingEndpointError::InvalidResponse("non-finite embedding value"),
            )?;
            vector.push(value as f32);
        }
        normalize(&mut vector, normalization)?;
        output[index] = Some(vector);
    }
    output
        .into_iter()
        .collect::<Option<Vec<_>>>()
        .ok_or(EmbeddingEndpointError::InvalidResponse(
            "missing indexed embedding",
        ))
}

fn normalize(
    vector: &mut [f32],
    normalization: EmbeddingNormalization,
) -> Result<(), EmbeddingEndpointError> {
    if normalization == EmbeddingNormalization::None {
        return Ok(());
    }
    let norm = vector
        .iter()
        .map(|value| (*value as f64) * (*value as f64))
        .sum::<f64>()
        .sqrt();
    if norm == 0.0 || !norm.is_finite() {
        return Err(EmbeddingEndpointError::InvalidResponse(
            "zero embedding vector",
        ));
    }
    for value in vector {
        *value = (*value as f64 / norm) as f32;
    }
    Ok(())
}
