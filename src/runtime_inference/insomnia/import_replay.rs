//! Insomnia-owned interpretation of accepted import replay records.
pub(crate) fn decode_completion(
    payload: &[u8],
) -> Result<Option<super::completion::InsomniaCompletion>, &'static str> {
    super::completion::decode_completion(payload)
}
pub(crate) fn encode_completion(
    completion: &super::completion::InsomniaCompletion,
) -> Result<Vec<u8>, &'static str> {
    super::completion::encode_completion(completion)
}
pub(crate) fn decode_record(
    payload: &[u8],
) -> Result<Option<super::codec::InsomniaRecord>, super::InsomniaError> {
    super::codec::decode_record(payload)
}
