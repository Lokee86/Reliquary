use super::validation::{check_revision, next_revision};
use super::{EgoRecord, EgoStore};
use crate::ego_codec::validate_text;
use crate::{Container, EgoError, EgoPersonality, EgoWebSynthesis};

impl EgoStore {
    pub(crate) fn put_personality(
        &mut self,
        container: &mut Container,
        expected_revision: u64,
        text: String,
        source_memory_version: u64,
    ) -> Result<(EgoPersonality, bool), EgoError> {
        self.require_phylactery("Personality is PHY-owned")?;
        validate_text("personality", &text)?;
        let actual_revision = self.personality.as_ref().map_or(0, |value| value.revision);
        check_revision(expected_revision, actual_revision)?;
        if self.personality.as_ref().is_some_and(|value| {
            value.text == text && value.source_memory_version == source_memory_version
        }) {
            return Ok((self.personality.clone().unwrap(), false));
        }
        let revision = next_revision(actual_revision)?;
        let record = EgoRecord::Personality {
            ego_version: self.next_version()?,
            revision,
            source_memory_version,
            text: text.clone(),
        };
        self.append_apply(container, record)?;
        Ok((
            EgoPersonality {
                revision,
                text,
                source_memory_version,
            },
            true,
        ))
    }

    pub(crate) fn put_synthesis(
        &mut self,
        container: &mut Container,
        expected_revision: u64,
        source_memory_version: u64,
        text: String,
    ) -> Result<(EgoWebSynthesis, bool), EgoError> {
        validate_text("synthesis", &text)?;
        let actual_revision = self.synthesis.as_ref().map_or(0, |value| value.revision);
        check_revision(expected_revision, actual_revision)?;
        if self.synthesis.as_ref().is_some_and(|value| {
            value.text == text && value.source_memory_version == source_memory_version
        }) {
            return Ok((self.synthesis.clone().unwrap(), false));
        }
        let revision = next_revision(actual_revision)?;
        let record = EgoRecord::Synthesis {
            ego_version: self.next_version()?,
            revision,
            source_memory_version,
            text: text.clone(),
        };
        self.append_apply(container, record)?;
        Ok((
            EgoWebSynthesis {
                revision,
                source_memory_version,
                text,
            },
            true,
        ))
    }
}
