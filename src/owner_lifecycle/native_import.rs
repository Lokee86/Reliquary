//! Concrete offline native Import producer access.
use crate::imports::ImportEpochStore;
use crate::{EpisodeConfig, EpisodeOrigin};
pub(crate) struct NativeImportParts<'a> {
    container: &'a mut crate::Container,
    pub(crate) archive: &'a mut crate::Archive,
    project_files: &'a crate::project_file_binding_store::ProjectFileStore,
    insomnia: &'a mut crate::insomnia::store::InsomniaStore,
    pub(crate) epochs: &'a mut ImportEpochStore,
}
impl<'a> NativeImportParts<'a> {
    pub(crate) fn new(
        container: &'a mut crate::Container,
        archive: &'a mut crate::Archive,
        project_files: &'a crate::project_file_binding_store::ProjectFileStore,
        insomnia: &'a mut crate::insomnia::store::InsomniaStore,
        epochs: &'a mut ImportEpochStore,
    ) -> Self {
        Self {
            container,
            archive,
            project_files,
            insomnia,
            epochs,
        }
    }
    pub(crate) fn owner_uuid(&self) -> Option<[u8; 16]> {
        self.container.owner_uuid()
    }
    pub(crate) fn sync(&self) -> Result<(), crate::ContainerError> {
        self.container.sync()
    }
    pub(crate) fn enter_commit(&mut self) {
        self.container.set_owner_write_scope(true);
    }
    pub(crate) fn leave_commit(&mut self) {
        self.container.set_owner_write_scope(false);
    }
    pub(crate) fn append_start(
        &mut self,
        start: crate::imports::ImportStart,
    ) -> Result<(), crate::imports::ImportError> {
        self.epochs.append_start(self.container, start)
    }
    pub(crate) fn ingest_turn(
        &mut self,
        turn: crate::IncomingTurn,
    ) -> Result<u64, crate::ArchiveError> {
        self.archive
            .ingest_turn(self.container, self.project_files.accepted_facts(), turn)?;
        Ok(self.archive.rel_turn_count())
    }
    pub(crate) fn put_metadata(
        &mut self,
        metadata: crate::ConversationMetadata,
    ) -> Result<bool, crate::ArchiveError> {
        self.archive
            .put_conversation_metadata(self.container, metadata)
    }
    pub(crate) fn append_branch(
        &mut self,
        branch: crate::Branch,
    ) -> Result<(), crate::ArchiveError> {
        self.archive.append_branch(self.container, branch)
    }
    pub(crate) fn put_receipt(
        &mut self,
        receipt: crate::imports::ImportReceipt,
    ) -> Result<(), crate::ArchiveError> {
        self.archive
            .put_import_receipt(self.container, receipt)
            .map(|_| ())
    }

    pub(crate) fn finalize_import_path_and_queue(
        &mut self,
        conversation_id: &str,
        leaf: &str,
        config: EpisodeConfig,
        now_ns: i64,
    ) -> Result<(), crate::InsomniaError> {
        let episodes = self
            .archive
            .materialize_path_episodes(
                self.container,
                conversation_id,
                leaf,
                config,
                EpisodeOrigin::Import,
                Some((crate::EpisodeBoundary::ImportEnd, now_ns)),
            )
            .map_err(|_| crate::InsomniaError::InvalidField("import episode"))?;
        for episode in episodes.created {
            self.queue_import_episode(episode.id, now_ns)?;
        }
        Ok(())
    }
    pub(crate) fn queue_import_episode(
        &mut self,
        id: crate::EpisodeId,
        now_ns: i64,
    ) -> Result<(), crate::InsomniaError> {
        let episode = self
            .archive
            .episode(id)
            .ok_or(crate::InsomniaError::MissingEpisode)?;
        let source = crate::owner_lifecycle::insomnia_replay::episode_source(
            &episode,
            crate::InsomniaPriority::Import,
        );
        self.insomnia.queue(self.container, &source, now_ns)?;
        Ok(())
    }
}
