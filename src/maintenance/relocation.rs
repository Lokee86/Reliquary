//! Relocation of supported durable owner payloads during transformed-copy publication.
use crate::ObjectRef;
use std::collections::BTreeMap;

pub(crate) fn relocate_payload(
    payload: &[u8],
    relocated: &BTreeMap<ObjectRef, ObjectRef>,
) -> Result<Vec<u8>, String> {
    if let Some(mut version) = crate::vector_generation_codec::decode_version(payload)
        .map_err(|error| error.to_string())?
    {
        version.record = relocated_ref(version.record, relocated, "Vector generation record")?;
        return Ok(crate::vector_generation_codec::encode_version(version).to_vec());
    }
    if let Some(envelope) = crate::container::publication_envelope::decode(payload)
        .map_err(|error| error.to_string())?
    {
        // These owner bodies contain stable IDs or Graph ordinals, never
        // physical ObjectRefs. Validate the supported body and preserve the
        // complete envelope byte-for-byte; direct Graph mappings are retained.
        if crate::memory_compact_codec::publication::decode(envelope.body, envelope.global_version)
            .map_err(|error| error.to_string())?
            .is_none()
            && crate::entity_compact_codec::publication::decode(
                envelope.body,
                envelope.global_version,
            )
            .map_err(|error| error.to_string())?
            .is_none()
            && crate::relationship_compact_codec::publication::decode(
                envelope.body,
                envelope.global_version,
            )
            .map_err(|error| error.to_string())?
            .is_none()
            && crate::archive_codec::publication::decode(envelope.body, envelope.global_version)
                .map_err(|error| error.to_string())?
                .is_none()
            && crate::graph_compact_codec::publication::decode(
                envelope.body,
                envelope.global_version,
            )
            .map_err(|error| error.to_string())?
            .is_none()
        {
            return Err("unsupported publication envelope body during relocation".to_owned());
        }
        return Ok(payload.to_vec());
    }
    Ok(payload.to_vec())
}

fn relocated_ref(
    source: ObjectRef,
    relocated: &BTreeMap<ObjectRef, ObjectRef>,
    kind: &str,
) -> Result<ObjectRef, String> {
    relocated
        .get(&source)
        .copied()
        .ok_or_else(|| format!("{kind} points to a missing or not-yet-copied physical object"))
}
