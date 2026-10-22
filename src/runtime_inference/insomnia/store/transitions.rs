use super::{InsomniaStore, clear_lease};
use crate::insomnia::codec::encode_work;
use crate::{
    Container, EpisodeId, InsomniaError, InsomniaLeaseToken, InsomniaWork, InsomniaWorkState,
};

impl InsomniaStore {
    pub(crate) fn fail(
        &mut self,
        _container: &mut Container,
        episode_id: EpisodeId,
        token: InsomniaLeaseToken,
        _started_at_ns: i64,
        failed_at_ns: i64,
        retry_after_ns: i64,
        reason: String,
    ) -> Result<InsomniaWork, InsomniaError> {
        if reason.trim().is_empty() || retry_after_ns <= failed_at_ns {
            return Err(InsomniaError::InvalidField("retry failure"));
        }
        let mut work = self.active_claim(episode_id, token, failed_at_ns)?;
        work.state = InsomniaWorkState::Failed;
        clear_lease(&mut work);
        work.retry_after_ns = Some(retry_after_ns);
        work.last_error = Some(reason);
        work.updated_at_ns = failed_at_ns;
        self.replace_work(work.clone())?;
        Ok(work)
    }

    pub(crate) fn terminal(
        &mut self,
        container: &mut Container,
        episode_id: EpisodeId,
        token: InsomniaLeaseToken,
        _started_at_ns: i64,
        terminal_at_ns: i64,
        reason: String,
    ) -> Result<InsomniaWork, InsomniaError> {
        if reason.trim().is_empty() {
            return Err(InsomniaError::InvalidField("terminal reason"));
        }
        let mut work = self.active_claim(episode_id, token, terminal_at_ns)?;
        work.state = InsomniaWorkState::Terminal;
        clear_lease(&mut work);
        work.retry_after_ns = None;
        work.last_error = Some(reason);
        work.updated_at_ns = terminal_at_ns;
        container.append(&encode_work(&work)?)?;
        self.replace_work(work.clone())?;
        Ok(work)
    }

    pub(crate) fn retry_terminal(
        &mut self,
        container: &mut Container,
        episode_id: EpisodeId,
        now_ns: i64,
    ) -> Result<InsomniaWork, InsomniaError> {
        let current = self
            .work(episode_id)
            .cloned()
            .ok_or(InsomniaError::MissingWork)?;
        if current.state != InsomniaWorkState::Terminal {
            return Err(InsomniaError::InvalidTransition);
        }
        let mut work = current;
        work.state = InsomniaWorkState::Pending;
        work.attempt_count = 0;
        clear_lease(&mut work);
        work.retry_after_ns = None;
        work.last_error = None;
        work.updated_at_ns = now_ns;
        container.append(&encode_work(&work)?)?;
        self.replace_work(work.clone())?;
        Ok(work)
    }
}
