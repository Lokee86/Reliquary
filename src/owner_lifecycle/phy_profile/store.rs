use super::{
    codec::{decode_phylactery_profile, encode_phylactery_profile},
    model::PhylacteryProfile,
};
use crate::Container;

#[derive(Default)]
pub(crate) struct PhylacteryProfileStore {
    current: Option<PhylacteryProfile>,
}

impl PhylacteryProfileStore {
    pub(crate) fn ingest(&mut self, payload: &[u8]) -> Result<(), String> {
        if let Some(profile) = decode_phylactery_profile(payload)? {
            self.current = Some(profile);
        }
        Ok(())
    }

    pub(crate) fn current(&self) -> PhylacteryProfile {
        self.current.clone().unwrap_or_default()
    }

    pub(crate) fn put(
        &mut self,
        container: &mut Container,
        profile: PhylacteryProfile,
    ) -> Result<bool, String> {
        if self.current.as_ref() == Some(&profile) {
            return Ok(false);
        }
        container
            .append(&encode_phylactery_profile(&profile)?)
            .map_err(|error| error.to_string())?;
        self.current = Some(profile);
        Ok(true)
    }
}
