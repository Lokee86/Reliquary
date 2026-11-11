use super::*;
use reliquary_application::inspection::{
    InspectionCursor, InspectionCut, InspectionItem, InspectionPage, InspectionQuery,
    MemoryListFilters, MountedOwnerInfo, SemanticGraphProjectionItem,
    SemanticGraphProjectionStatus, SourceAvailability, finish_provenance, inspect, inspection_cut,
    memory_matches_filter, mounted_owner_info, resolve_provenance, visit_semantic_graph_projection,
};
use reliquary_owner_session::{
    CommunityId, ManagementRead, MemoryId, SemanticGraphCommunityMemberPage,
    SemanticGraphCommunityPage, SemanticGraphCut, SemanticGraphMemoryPage,
    SemanticGraphRelationPage,
};

impl Instance {
    pub fn semantic_graph_inspection_cut(
        &self,
        owner: OwnerId,
        phy: bool,
    ) -> Result<InspectionCut, GatewayError> {
        let cut = self.with_management_read(owner, phy, |read| inspection_cut(&read))?;
        self.ensure_credential_active()?;
        Ok(cut)
    }

    pub fn memory_matches_filter(
        &self,
        owner: OwnerId,
        phy: bool,
        expected_cut: InspectionCut,
        filters: &MemoryListFilters,
        id: MemoryId,
    ) -> Result<bool, GatewayError> {
        let matches = self
            .with_management_read(owner, phy, |read| {
                memory_matches_filter(read, expected_cut, filters, id)
            })?
            .map_err(management_error)?;
        self.ensure_credential_active()?;
        Ok(matches)
    }

    pub fn visit_semantic_graph_projection(
        &self,
        owner: OwnerId,
        phy: bool,
        expected_cut: InspectionCut,
        filters: &MemoryListFilters,
        visit: impl FnMut(SemanticGraphProjectionItem) -> std::ops::ControlFlow<()>,
    ) -> Result<SemanticGraphProjectionStatus, GatewayError> {
        let status = self
            .with_management_read(owner, phy, |read| {
                visit_semantic_graph_projection(read, expected_cut, filters, visit)
            })?
            .map_err(management_error)?;
        self.ensure_credential_active()?;
        Ok(status)
    }

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

    pub fn semantic_graph_memory_page(
        &self,
        owner: OwnerId,
        phy: bool,
        expected_cut: Option<SemanticGraphCut>,
        after: Option<MemoryId>,
        limit: usize,
    ) -> Result<(SemanticGraphCut, SemanticGraphMemoryPage), GatewayError> {
        self.with_management_read(owner, phy, |read| {
            let snapshot = read.semantic_graph_snapshot();
            let cut = snapshot.cut();
            if expected_cut.is_some_and(|expected| expected != cut) {
                return Err(GatewayError::OwnerSession(
                    OwnerSessionError::ManagementStale,
                ));
            }
            Ok((cut, snapshot.memory_page(after, limit)?))
        })?
    }

    pub fn semantic_graph_relation_page(
        &self,
        owner: OwnerId,
        phy: bool,
        expected_cut: SemanticGraphCut,
        offset: usize,
        limit: usize,
    ) -> Result<(SemanticGraphCut, SemanticGraphRelationPage), GatewayError> {
        self.with_management_read(owner, phy, |read| {
            let snapshot = read.semantic_graph_snapshot();
            let cut = snapshot.cut();
            if expected_cut != cut {
                return Err(GatewayError::OwnerSession(
                    OwnerSessionError::ManagementStale,
                ));
            }
            Ok((cut, snapshot.relation_page(offset, limit)?))
        })?
    }

    pub fn semantic_graph_community_page(
        &self,
        owner: OwnerId,
        phy: bool,
        expected_cut: SemanticGraphCut,
        after: Option<CommunityId>,
        limit: usize,
    ) -> Result<(SemanticGraphCut, SemanticGraphCommunityPage), GatewayError> {
        self.with_management_read(owner, phy, |read| {
            let snapshot = read.semantic_graph_snapshot();
            let cut = snapshot.cut();
            if expected_cut != cut {
                return Err(GatewayError::OwnerSession(
                    OwnerSessionError::ManagementStale,
                ));
            }
            Ok((cut, snapshot.community_page(after, limit)?))
        })?
    }

    pub fn semantic_graph_community_member_page(
        &self,
        owner: OwnerId,
        phy: bool,
        expected_cut: SemanticGraphCut,
        community_id: CommunityId,
        after: Option<MemoryId>,
        limit: usize,
    ) -> Result<(SemanticGraphCut, SemanticGraphCommunityMemberPage), GatewayError> {
        self.with_management_read(owner, phy, |read| {
            let snapshot = read.semantic_graph_snapshot();
            let cut = snapshot.cut();
            if expected_cut != cut {
                return Err(GatewayError::OwnerSession(
                    OwnerSessionError::ManagementStale,
                ));
            }
            Ok((
                cut,
                snapshot.community_member_page(community_id, after, limit)?,
            ))
        })?
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
