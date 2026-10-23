//! Import's decoding seam for persisted Insomnia replay records.

pub(crate) fn decode_completion(
    payload: &[u8],
) -> Result<Option<crate::insomnia::completion::InsomniaCompletion>, &'static str> {
    crate::insomnia::completion::decode_completion(payload)
}

pub(crate) fn decode_record(
    payload: &[u8],
) -> Result<Option<crate::insomnia::codec::InsomniaRecord>, crate::insomnia::InsomniaError> {
    crate::insomnia::codec::decode_record(payload)
}
