//! Durable user names for exact, already published Community memberships.

use super::*;

const MAGIC: &[u8; 8] = b"RELCMN01";
pub const MAX_COMMUNITY_NAME_BYTES: usize = 256;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CommunityRename {
    pub request_key: [u8; 32],
    pub cut_digest: [u8; 32],
    pub id: CommunityId,
    pub expected_generation: u64,
    pub graph_version: u64,
    pub name: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CommunityRenameReceipt {
    pub request: CommunityRename,
    pub generation: u64,
    pub member_count: usize,
}

impl CommunityRenameReceipt {
    pub fn changed(&self) -> bool {
        self.generation != self.request.expected_generation
    }
}

#[derive(Default)]
pub(super) struct CommunityNames {
    names: BTreeMap<CommunityId, String>,
    requests: BTreeMap<[u8; 32], usize>,
    history: Vec<CommunityRenameReceipt>,
}

impl CommunityStore {
    /// The latest explicitly named value for an unchanged membership identity.
    pub fn user_name(&self, id: CommunityId) -> Option<&str> {
        self.names.names.get(&id).map(String::as_str)
    }

    pub fn rename_receipt(&self, key: &[u8; 32]) -> Option<&CommunityRenameReceipt> {
        self.names
            .requests
            .get(key)
            .map(|index| &self.names.history[*index])
    }

    pub fn rename_history(&self) -> &[CommunityRenameReceipt] {
        &self.names.history
    }

    pub fn rename(
        &mut self,
        container: &mut Container,
        graph: &GraphStore,
        request: CommunityRename,
    ) -> Result<CommunityRenameReceipt, GraphError> {
        if container.owner_id() != self.owner || container.identity() != self.identity {
            return Err(GraphError::WrongOwner);
        }
        graph.check_owner(container)?;
        if self.identity.file_kind != reliquary_container::FileKind::Reliquary {
            return Err(GraphError::InvalidCommunity(
                "user name requires a REL owner",
            ));
        }
        validate_name(&request.name)?;
        if let Some(accepted) = self.rename_receipt(&request.request_key) {
            return if accepted.request == request {
                Ok(accepted.clone())
            } else {
                Err(GraphError::CommunityRequestConflict)
            };
        }
        if request.graph_version != graph.graph_version() {
            return Err(GraphError::StaleCommunity);
        }
        let receipt = self.prepare_rename(request)?;
        let bytes = encode(self.owner, &receipt);
        container.append(&bytes)?;
        container.sync()?;
        self.apply_rename(receipt.clone());
        Ok(receipt)
    }

    pub(super) fn replay_rename(
        &mut self,
        owner: [u8; 16],
        request: CommunityRename,
        generation: u64,
    ) -> Result<(), GraphError> {
        if owner != self.owner {
            return Err(GraphError::WrongOwner);
        }
        if self.identity.file_kind != reliquary_container::FileKind::Reliquary {
            return Err(GraphError::CorruptRecord(
                "Community user name in non-REL owner",
            ));
        }
        if self.rename_receipt(&request.request_key).is_some() {
            return Err(GraphError::CorruptRecord(
                "duplicate Community rename request",
            ));
        }
        let receipt = self
            .prepare_rename(request)
            .map_err(|_| GraphError::CorruptRecord("Community rename precondition"))?;
        if generation != receipt.generation {
            return Err(GraphError::CorruptRecord("Community rename generation"));
        }
        self.apply_rename(receipt);
        Ok(())
    }

    fn prepare_rename(
        &self,
        request: CommunityRename,
    ) -> Result<CommunityRenameReceipt, GraphError> {
        let snapshot = self.current.as_ref().ok_or(GraphError::UnknownCommunity)?;
        if snapshot.generation != request.expected_generation
            || snapshot.graph_version != request.graph_version
        {
            return Err(GraphError::StaleCommunity);
        }
        let member_count = snapshot
            .communities
            .binary_search_by_key(&request.id, |community| community.id)
            .map(|index| snapshot.communities[index].members.len())
            .map_err(|_| GraphError::UnknownCommunity)?;
        let generation = if self.user_name(request.id) == Some(request.name.as_str()) {
            snapshot.generation
        } else {
            snapshot
                .generation
                .checked_add(1)
                .ok_or(GraphError::VersionExhausted)?
        };
        Ok(CommunityRenameReceipt {
            request,
            generation,
            member_count,
        })
    }

    fn apply_rename(&mut self, receipt: CommunityRenameReceipt) {
        if receipt.changed() {
            self.names
                .names
                .insert(receipt.request.id, receipt.request.name.clone());
            // A rename retains the membership snapshot. Only its accepted generation changes.
            self.current
                .as_mut()
                .expect("validated Community")
                .generation = receipt.generation;
            self.history
                .last_mut()
                .expect("validated Community history")
                .generation = receipt.generation;
        }
        self.names
            .requests
            .insert(receipt.request.request_key, self.names.history.len());
        self.names.history.push(receipt);
    }
}

pub fn validate_community_name(name: &str) -> Result<(), GraphError> {
    validate_name(name)
}

fn validate_name(name: &str) -> Result<(), GraphError> {
    if name.trim().is_empty()
        || name.len() > MAX_COMMUNITY_NAME_BYTES
        || name.chars().any(char::is_control)
    {
        return Err(GraphError::InvalidCommunity("invalid user name"));
    }
    Ok(())
}

fn encode(owner: [u8; 16], receipt: &CommunityRenameReceipt) -> Vec<u8> {
    let request = &receipt.request;
    let mut bytes = MAGIC.to_vec();
    bytes.extend(owner);
    bytes.extend(request.request_key);
    bytes.extend(request.cut_digest);
    bytes.extend(request.id.0);
    bytes.extend(request.expected_generation.to_le_bytes());
    bytes.extend(receipt.generation.to_le_bytes());
    bytes.extend(request.graph_version.to_le_bytes());
    bytes.extend(request.name.as_bytes());
    bytes
}

pub(super) fn decode(bytes: &[u8]) -> Result<Option<([u8; 16], CommunityRename, u64)>, GraphError> {
    if !bytes.starts_with(MAGIC) {
        return Ok(None);
    }
    let mut position = MAGIC.len();
    let owner = take::<16>(bytes, &mut position)?;
    let request_key = take::<32>(bytes, &mut position)?;
    let cut_digest = take::<32>(bytes, &mut position)?;
    let id = CommunityId(take::<32>(bytes, &mut position)?);
    let expected_generation = u64::from_le_bytes(take::<8>(bytes, &mut position)?);
    let generation = u64::from_le_bytes(take::<8>(bytes, &mut position)?);
    let graph_version = u64::from_le_bytes(take::<8>(bytes, &mut position)?);
    let name = std::str::from_utf8(&bytes[position..])
        .map_err(|_| GraphError::CorruptRecord("Community rename name is not UTF-8"))?;
    validate_name(name).map_err(|_| GraphError::CorruptRecord("invalid Community rename name"))?;
    Ok(Some((
        owner,
        CommunityRename {
            request_key,
            cut_digest,
            id,
            expected_generation,
            graph_version,
            name: name.into(),
        },
        generation,
    )))
}

/// The exact physical Graph prefix of a user rename, including no-op receipts.
pub fn rename_graph_version_in_frame(payload: &[u8]) -> Result<Option<u64>, GraphError> {
    Ok(decode(payload)?.map(|(_, request, _)| request.graph_version))
}
