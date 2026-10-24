use crate::archive::ArchiveReadView;
use crate::freshness_dream_journal::DreamFreshnessJournal;
use crate::freshness_storage::FreshnessStore;
use crate::insomnia::store::InsomniaStore;
use crate::memory_store::MemoryStore;
use crate::{Archive, Container, InsomniaError, InsomniaWork};
pub(crate) struct InsomniaRuntimePublication<'a> {
    container: &'a mut Container,
    pub(crate) archive: &'a Archive,
    memories: Option<&'a mut MemoryStore>,
    insomnia: &'a mut InsomniaStore,
    freshness: Option<&'a mut FreshnessStore>,
    dream_freshness: Option<&'a mut DreamFreshnessJournal>,
}
impl<'a> InsomniaRuntimePublication<'a> {
    pub(crate) fn lease(
        container: &'a mut Container,
        archive: &'a Archive,
        insomnia: &'a mut InsomniaStore,
    ) -> Self {
        Self {
            container,
            archive,
            insomnia,
            memories: None,
            freshness: None,
            dream_freshness: None,
        }
    }
    pub(crate) fn completion(
        container: &'a mut Container,
        archive: &'a Archive,
        memories: &'a mut MemoryStore,
        insomnia: &'a mut InsomniaStore,
        freshness: &'a mut FreshnessStore,
        dream_freshness: &'a mut DreamFreshnessJournal,
    ) -> Self {
        Self {
            container,
            archive,
            memories: Some(memories),
            insomnia,
            freshness: Some(freshness),
            dream_freshness: Some(dream_freshness),
        }
    }
    pub(crate) fn read(&self) -> ArchiveReadView<'_> {
        ArchiveReadView::new(self.archive, self.container)
    }
    pub(crate) fn owner_id(&self) -> Option<String> {
        self.container.owner_id()
    }
    pub(crate) fn claim(
        &mut self,
        worker: &str,
        now: i64,
        duration: i64,
    ) -> Result<Option<InsomniaWork>, InsomniaError> {
        self.insomnia
            .claim_next(self.container, worker, now, duration)
    }
    pub(crate) fn renew(
        &mut self,
        work: &InsomniaWork,
        now: i64,
        duration: i64,
    ) -> Result<(), InsomniaError> {
        self.insomnia
            .renew(
                self.container,
                work.episode_id,
                work.lease_token.ok_or(InsomniaError::InvalidLease)?,
                now,
                duration,
            )
            .map(|_| ())
    }
    pub(crate) fn terminal(
        &mut self,
        work: &InsomniaWork,
        start: i64,
        end: i64,
        reason: String,
    ) -> Result<(), InsomniaError> {
        self.insomnia
            .terminal(
                self.container,
                work.episode_id,
                work.lease_token.ok_or(InsomniaError::InvalidLease)?,
                start,
                end,
                reason,
            )
            .map(|_| ())
    }
    pub(crate) fn fail(
        &mut self,
        work: &InsomniaWork,
        start: i64,
        end: i64,
        retry: i64,
        reason: String,
    ) -> Result<(), InsomniaError> {
        self.insomnia
            .fail(
                self.container,
                work.episode_id,
                work.lease_token.ok_or(InsomniaError::InvalidLease)?,
                start,
                end,
                retry,
                reason,
            )
            .map(|_| ())
    }
    pub(crate) fn commit(
        &mut self,
        claim: &InsomniaWork,
        prepared: crate::insomnia::PreparedApplication,
        publication: crate::insomnia::UserPublication,
        start: i64,
        end: i64,
    ) -> Result<crate::InsomniaProcessResult, crate::InsomniaProcessError> {
        let owner_uuid = self.container.owner_uuid();
        let turn = self.archive.rel_turn_count();
        super::insomnia_application::commit_application(
            self.container,
            self.memories.as_deref_mut().expect("completion capability"),
            self.insomnia,
            self.freshness
                .as_deref_mut()
                .expect("completion capability"),
            self.dream_freshness
                .as_deref_mut()
                .expect("completion capability"),
            owner_uuid,
            turn,
            claim,
            prepared,
            Some(publication),
            start,
            end,
        )
    }
}
