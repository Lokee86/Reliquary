use crate::{Container, ContainerIdentity, FileKind, Phylactery, PhylacteryError};
use std::path::Path;

impl Phylactery {
    pub fn create(path: impl AsRef<Path>) -> Result<Self, PhylacteryError> {
        let container = Container::create_with_identity(
            path,
            ContainerIdentity {
                file_kind: FileKind::Phylactery,
                scope: None,
            },
        )?;
        Self::from_owner(crate::owner_lifecycle::initialize_phylactery(container)?)
    }

    pub(crate) fn create_with_uuid(
        path: impl AsRef<Path>,
        owner_uuid: [u8; 16],
    ) -> Result<Self, PhylacteryError> {
        let container = Container::create_with_identity_and_uuid(
            path,
            ContainerIdentity {
                file_kind: FileKind::Phylactery,
                scope: None,
            },
            owner_uuid,
        )?;
        Self::from_owner(crate::owner_lifecycle::initialize_phylactery(container)?)
    }

    fn from_owner(owner: crate::owner_lifecycle::PhylacteryOwner) -> Result<Self, PhylacteryError> {
        let crate::owner_lifecycle::PhylacteryOwner {
            owner_maintenance,
            rewrite_lineage,
            container,
            memories,
            graph,
            communities,
            dream_pairs,
            processing_epochs,
            packed_vectors,
            memory_vectors,
            compatibility_profiles,
            profile,
            ego,
            entities,
            entity_resolutions,
            relationships,
        } = owner;
        Ok(Self {
            owner_maintenance,
            rewrite_lineage,
            container,
            memories,
            lexical_index: crate::lexical_index::LexicalIndex::default(),
            graph,
            communities,
            duplicate_index: crate::dream_duplicate_index::DuplicateIndex::empty(),
            processing_epochs,
            dream_pairs,
            packed_vectors,
            memory_vectors,
            compatibility_profiles,
            profile,
            ego,
            entities,
            entity_resolutions,
            relationships,
        })
    }

    pub fn open(path: impl AsRef<Path>) -> Result<Self, PhylacteryError> {
        let container = Container::open(path)?;
        Self::open_from_container(container)
    }

    /// Replays a coherent read-only snapshot without acquiring writer authority.
    pub fn open_read_only(path: impl AsRef<Path>) -> Result<Self, PhylacteryError> {
        Self::open_from_container(Container::open_read_only(path)?)
    }

    pub(crate) fn open_from_container(container: Container) -> Result<Self, PhylacteryError> {
        let mut result = Self::from_owner(crate::owner_lifecycle::open_phylactery_from_container(
            container,
        )?)?;

        // These are the original post-replay checkpoints. History-backed prune
        // proof and the perception-owned historical resolution references stay
        // above the lower owner lifecycle aggregate.
        result.validate_prune_evidence()?;
        result.entity_resolutions.validate_local()?;
        crate::owner_lifecycle::validate_entity_resolution_references(
            &result.memories,
            &result.entities,
            &result.entity_resolutions.records(),
        )?;
        Ok(result)
    }
}
