use reliquary_owner_session::records::external_index::model::{
    ExternalChunkRef, ExternalMessageDescriptor, ExternalSourceError, MAX_EXTERNAL_BATCH,
    MAX_EXTERNAL_MESSAGE_BYTES,
};
use sha2::{Digest, Sha256};

/// Host text is transient and always paired with its canonical descriptor.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HydratedExternalMessage {
    pub descriptor: ExternalMessageDescriptor,
    pub content: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HydratedExternalChunk {
    pub reference: ExternalChunkRef,
    pub content: String,
}

impl HydratedExternalMessage {
    pub fn into_chunk(
        self,
        reference: ExternalChunkRef,
    ) -> Result<HydratedExternalChunk, ExternalSourceError> {
        if self.descriptor.reference != reference.message {
            return Err(ExternalSourceError::StaleEvidence);
        }
        verify_external_hydration(
            &reference.message.source_namespace,
            std::slice::from_ref(&self.descriptor),
            std::slice::from_ref(&self),
        )?;
        let start = usize::try_from(reference.start).map_err(|_| ExternalSourceError::Bounds)?;
        let end = usize::try_from(reference.end).map_err(|_| ExternalSourceError::Bounds)?;
        if start >= end {
            return Err(ExternalSourceError::Bounds);
        }
        let content = self
            .content
            .get(start..end)
            .ok_or(ExternalSourceError::Bounds)?
            .to_owned();
        Ok(HydratedExternalChunk { reference, content })
    }
}

pub fn validate_external_hydration_request(
    namespace: &str,
    expected: &[ExternalMessageDescriptor],
) -> Result<(), ExternalSourceError> {
    if expected.is_empty() || expected.len() > MAX_EXTERNAL_BATCH {
        return Err(ExternalSourceError::Bounds);
    }
    validate_descriptors(namespace, expected)
}

fn validate_descriptors(
    namespace: &str,
    expected: &[ExternalMessageDescriptor],
) -> Result<(), ExternalSourceError> {
    reliquary_owner_session::records::external_index::model::identifier(namespace)?;
    let mut identities = std::collections::BTreeSet::new();
    for descriptor in expected {
        descriptor.validate(namespace)?;
        if !descriptor.active
            || !identities.insert((
                &descriptor.reference.conversation_id,
                &descriptor.reference.message_id,
            ))
        {
            return Err(ExternalSourceError::InvalidContract);
        }
    }
    Ok(())
}

/// Confirm order, identity, visibility, and bytes against the requested host
/// snapshot. A successful check has no native Archive side effects.
pub fn verify_external_hydration(
    namespace: &str,
    expected: &[ExternalMessageDescriptor],
    hydrated: &[HydratedExternalMessage],
) -> Result<(), ExternalSourceError> {
    validate_external_hydration_request(namespace, expected)?;
    if expected.len() != hydrated.len() {
        return Err(ExternalSourceError::StaleEvidence);
    }
    for (descriptor, message) in expected.iter().zip(hydrated) {
        if message.descriptor != *descriptor
            || message.content.len() > MAX_EXTERNAL_MESSAGE_BYTES
            || <[u8; 32]>::from(Sha256::digest(message.content.as_bytes()))
                != descriptor.reference.content_hash
        {
            return Err(ExternalSourceError::StaleEvidence);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use reliquary_owner_session::records::external_index::ExternalMessageRef;

    fn message(id: &str, text: &str) -> HydratedExternalMessage {
        HydratedExternalMessage {
            descriptor: ExternalMessageDescriptor {
                reference: ExternalMessageRef {
                    source_namespace: "host".into(),
                    conversation_id: "conversation".into(),
                    message_id: id.into(),
                    content_hash: Sha256::digest(text.as_bytes()).into(),
                },
                role: "user".into(),
                principal_id: Some("alice".into()),
                timestamp_ns: 10,
                active: true,
            },
            content: text.into(),
        }
    }

    #[test]
    fn chunk_preserves_exact_utf8_range_and_message_identity() {
        let hydrated = message("a", "é 🦀");
        let mut reference = ExternalChunkRef {
            message: hydrated.descriptor.reference.clone(),
            start: 3,
            end: 7,
        };
        assert_eq!(
            hydrated
                .clone()
                .into_chunk(reference.clone())
                .unwrap()
                .content,
            "🦀"
        );
        reference.start = 4;
        assert_eq!(
            hydrated.clone().into_chunk(reference.clone()),
            Err(ExternalSourceError::Bounds)
        );
        reference.message.message_id = "other".into();
        assert_eq!(
            hydrated.into_chunk(reference),
            Err(ExternalSourceError::StaleEvidence)
        );
    }

    #[test]
    fn batch_requires_exact_visible_descriptors_in_order() {
        let hydrated = vec![message("a", "first"), message("b", "second")];
        let expected: Vec<_> = hydrated
            .iter()
            .map(|message| message.descriptor.clone())
            .collect();
        assert_eq!(
            verify_external_hydration("host", &expected, &hydrated),
            Ok(())
        );
        let mut swapped = hydrated.clone();
        swapped.swap(0, 1);
        assert_eq!(
            verify_external_hydration("host", &expected, &swapped),
            Err(ExternalSourceError::StaleEvidence)
        );
        let mut forged = hydrated.clone();
        forged[0].descriptor.principal_id = Some("bob".into());
        assert_eq!(
            verify_external_hydration("host", &expected, &forged),
            Err(ExternalSourceError::StaleEvidence)
        );
        let mut hidden = expected.clone();
        hidden[0].active = false;
        assert_eq!(
            validate_external_hydration_request("host", &hidden),
            Err(ExternalSourceError::InvalidContract)
        );
        assert_eq!(
            validate_external_hydration_request("foreign", &expected),
            Err(ExternalSourceError::InvalidContract)
        );
        let duplicate = vec![expected[0].clone(), expected[0].clone()];
        assert_eq!(
            validate_external_hydration_request("host", &duplicate),
            Err(ExternalSourceError::InvalidContract)
        );
    }
}
