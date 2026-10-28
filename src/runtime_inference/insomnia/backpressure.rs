use super::InsomniaExtractionError;
use crate::GeneralEndpointError;

pub const DEFAULT_INSOMNIA_BACKPRESSURE_DELAY_NS: i64 = 60 * 1_000_000_000;

pub(crate) fn retry_after_ns(error: &InsomniaExtractionError) -> Option<i64> {
    let retry_after = match error {
        InsomniaExtractionError::Endpoint(GeneralEndpointError::Backpressure {
            retry_after,
            ..
        }) => retry_after,
        InsomniaExtractionError::Decision(crate::DecisionEndpointError::Backpressure {
            retry_after,
            ..
        }) => retry_after,
        _ => return None,
    };
    let hinted = retry_after
        .map(|duration| i64::try_from(duration.as_nanos()).unwrap_or(i64::MAX))
        .unwrap_or(0);
    Some(hinted.max(DEFAULT_INSOMNIA_BACKPRESSURE_DELAY_NS))
}
