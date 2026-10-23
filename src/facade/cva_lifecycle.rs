use crate::dream_duplicate_index::DuplicateIndex;
use crate::lexical_index::LexicalIndex;
use crate::{Container, Cva, CvaError};
use std::path::Path;

impl Cva {
    pub fn create(path: impl AsRef<Path>) -> Result<Self, CvaError> {
        Self::create_rel(path, None)
    }

    pub fn create_project(path: impl AsRef<Path>) -> Result<Self, CvaError> {
        Self::create_rel(path, Some("Project"))
    }

    pub fn create_organization(path: impl AsRef<Path>) -> Result<Self, CvaError> {
        Self::create_rel(path, Some("Organization"))
    }

    pub fn create_connection(path: impl AsRef<Path>) -> Result<Self, CvaError> {
        Self::create_rel(path, Some("Connection"))
    }

    pub fn open_project(path: impl AsRef<Path>) -> Result<Self, CvaError> {
        Self::open(path)
    }

    pub fn open_organization(path: impl AsRef<Path>) -> Result<Self, CvaError> {
        Self::open(path)
    }

    pub fn open_connection(path: impl AsRef<Path>) -> Result<Self, CvaError> {
        Self::open(path)
    }

    fn create_rel(path: impl AsRef<Path>, type_label: Option<&str>) -> Result<Self, CvaError> {
        let container = Container::create_with_identity(
            path,
            crate::ContainerIdentity {
                file_kind: crate::FileKind::Reliquary,
                scope: None,
            },
        )?;
        let mut cva = Self::initialize(container)?;
        if let Some(type_label) = type_label {
            cva.set_rel_metadata(Some(type_label.to_owned()), Vec::new())?;
            cva.sync()?;
        }
        Ok(cva)
    }

    pub(crate) fn create_rel_with_uuid(
        path: impl AsRef<Path>,
        owner_uuid: [u8; 16],
        type_label: Option<String>,
    ) -> Result<Self, CvaError> {
        let container = Container::create_with_identity_and_uuid(
            path,
            crate::ContainerIdentity {
                file_kind: crate::FileKind::Reliquary,
                scope: None,
            },
            owner_uuid,
        )?;
        let mut cva = Self::initialize(container)?;
        if type_label.is_some() {
            cva.set_rel_metadata(type_label, Vec::new())?;
        }
        Ok(cva)
    }

    pub(crate) fn create_legacy_scope_with_uuid(
        path: impl AsRef<Path>,
        scope: crate::ReliquaryScopeKind,
        owner_uuid: [u8; 16],
    ) -> Result<Self, CvaError> {
        let container = Container::create_with_identity_and_uuid(
            path,
            crate::ContainerIdentity {
                file_kind: crate::FileKind::Reliquary,
                scope: Some(scope),
            },
            owner_uuid,
        )?;
        Self::initialize(container)
    }

    fn initialize(container: Container) -> Result<Self, CvaError> {
        Ok(Self::from_owner(
            crate::owner_lifecycle::initialize_reliquary(container)?,
        ))
    }

    fn from_owner(owner: crate::owner_lifecycle::ReliquaryOwner) -> Self {
        let crate::owner_lifecycle::ReliquaryOwner {
            owner_maintenance,
            rewrite_lineage,
            external_index,
            container,
            archive,
            memories,
            graph,
            communities,
            processing_epochs,
            dream_pairs,
            insomnia,
            import_epochs,
            packed_vectors,
            memory_vectors,
            archive_vectors,
            compatibility_profiles,
            vector_generations,
            conversation_compactions,
            interaction_streams,
            echo,
            ego,
            entities,
            entity_resolutions,
            relationships,
            project_history,
            project_files,
            rel_metadata,
            freshness,
            dream_freshness,
        } = owner;
        Self {
            owner_maintenance,
            rewrite_lineage,
            external_index,
            container,
            archive,
            memories,
            graph,
            communities,
            processing_epochs,
            dream_pairs,
            insomnia,
            import_epochs,
            packed_vectors,
            memory_vectors,
            archive_vectors,
            compatibility_profiles,
            vector_generations,
            conversation_compactions,
            interaction_streams,
            echo,
            ego,
            entities,
            entity_resolutions,
            relationships,
            project_history,
            project_files,
            rel_metadata,
            freshness,
            dream_freshness,
            duplicate_index: DuplicateIndex::empty(),
            lexical_index: LexicalIndex::default(),
            freshness_due: crate::FreshnessDueDispatcher::default(),
            freshness_notifications: std::collections::BTreeMap::new(),
        }
    }

    pub fn open(path: impl AsRef<Path>) -> Result<Self, CvaError> {
        let container = Container::open(path)?;
        Self::open_from_container(container)
    }

    /// Replays a coherent read-only snapshot without acquiring writer authority.
    pub fn open_read_only(path: impl AsRef<Path>) -> Result<Self, CvaError> {
        Self::open_from_container(Container::open_read_only(path)?)
    }

    pub(crate) fn open_from_container(container: Container) -> Result<Self, CvaError> {
        let mut replay = crate::owner_lifecycle::ReliquaryReplay::read(container)?;
        let reconstruction = crate::history::reconstruct_reliquary(&mut replay)?;
        let (owner, due) = replay.into_owner(reconstruction);
        let mut result = Self::from_owner(owner);
        result.freshness_due = due;
        Ok(result)
    }
}
