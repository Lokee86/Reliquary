use super::*;
use reliquary_application::inspection::{
    InspectionCursor, InspectionItem, InspectionPage, InspectionQuery, MountedOwnerInfo,
    SourceAvailability, finish_provenance, inspect, mounted_owner_info, resolve_provenance,
};
use reliquary_owner_session::ManagementRead;

impl Instance {
    pub fn inspect_management(
        &self,
        owner: OwnerId,
        phy: bool,
        query: &InspectionQuery,
        cursor: Option<&InspectionCursor>,
        limit: usize,
    ) -> Result<InspectionPage, GatewayError> {
        let mut page = self
            .with_management_read(owner, phy, |read| inspect(read, query, cursor, limit))?
            .map_err(management_error)?;
        if let [InspectionItem::Provenance(result)] = page.items.as_mut_slice() {
            if let Some(source) = result.source_owner().and_then(owner_id_from_hex) {
                if source != owner && self.rel_grants.contains(&source) {
                    match self.with_reliquary(source, |rel| resolve_provenance(rel, result)) {
                        Ok(resolved) => resolved.map_err(management_error)?,
                        Err(GatewayError::UnknownOwner(_)) => {
                            result.availability = SourceAvailability::Unavailable;
                        }
                        Err(error) => return Err(error),
                    }
                }
            }
            finish_provenance(&mut page).map_err(management_error)?;
        }
        self.ensure_credential_active()?;
        Ok(page)
    }

    pub fn mounted_management_info(
        &self,
        owner: OwnerId,
        phy: bool,
    ) -> Result<MountedOwnerInfo, GatewayError> {
        self.with_management_read(owner, phy, mounted_owner_info)
    }

    pub(crate) fn with_management_read<T>(
        &self,
        owner: OwnerId,
        phy: bool,
        read: impl FnOnce(ManagementRead<'_>) -> T,
    ) -> Result<T, GatewayError> {
        if !phy {
            return self.with_reliquary(owner, |session| read(session.management_read()));
        }
        self.ensure_credential_active()?;
        if !self.phy_grants.contains(&owner) {
            return Err(GatewayError::UnauthorizedPhy {
                principal: self.principal.to_string(),
                owner,
            });
        }
        let mounted = self
            .owners
            .read()
            .map_err(|_| GatewayError::RegistryPoisoned)?
            .get(&owner)
            .cloned()
            .ok_or(GatewayError::UnknownOwner(owner))?;
        match mounted {
            MountedOwner::Phy(session) => Ok(read(session.management_read())),
            MountedOwner::PhyWriter(mount) => {
                let writer = mount
                    .writer
                    .lock()
                    .map_err(|_| GatewayError::RegistryPoisoned)?;
                self.ensure_credential_active()?;
                Ok(read(writer.management_read()?))
            }
            _ => Err(GatewayError::WrongOwnerKind(owner)),
        }
    }
}

fn owner_id_from_hex(value: &str) -> Option<OwnerId> {
    if value.len() != 32 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return None;
    }
    let mut id = [0; 16];
    for (index, byte) in id.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&value[index * 2..index * 2 + 2], 16).ok()?;
    }
    Some(OwnerId::from_bytes(id))
}
