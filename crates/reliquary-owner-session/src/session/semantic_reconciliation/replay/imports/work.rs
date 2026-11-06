//! Replay independently validated imported Insomnia work journals through
//! the destination owner's transition APIs, preserving every accepted revision.
use super::*;
use reliquary_insomnia_store::{InsomniaLeaseToken, InsomniaStore, InsomniaWorkState};

pub(super) fn replay_before_completion(
    writer: &mut RelWriter,
    source: &Container,
    episode: EpisodeId,
    import_id: &str,
) -> Result<InsomniaLeaseToken, RelReconciliationError> {
    let id = WorkEpisodeId(episode.0);
    let history = InsomniaStore::work_history(source, id).map_err(conflict)?;
    if !(2..=256).contains(&history.len()) {
        return Err(RelReconciliationError::UnsupportedState(
            "import Insomnia history exceeds supported journal replay",
        ));
    }
    if writer.session.insomnia.work(id) != history.first() {
        return Err(conflict(
            "destination imported work does not match its source queue",
        ));
    }
    for expected in history.iter().skip(1) {
        let prior = writer
            .session
            .insomnia
            .work(id)
            .cloned()
            .ok_or_else(|| conflict("import Insomnia work disappeared during replay"))?;
        let actual = match (prior.state, expected.state) {
            (
                InsomniaWorkState::Pending | InsomniaWorkState::Failed,
                InsomniaWorkState::Processing,
            ) => {
                let worker = expected
                    .lease_owner
                    .as_deref()
                    .ok_or_else(|| conflict("source Insomnia claim lacks worker identity"))?;
                let duration = expected
                    .lease_expires_ns
                    .and_then(|expires| expires.checked_sub(expected.updated_at_ns))
                    .filter(|value| *value > 0)
                    .ok_or_else(|| conflict("source Insomnia claim has invalid lease"))?;
                writer
                    .claim_native_import_episode(
                        import_id,
                        episode,
                        worker,
                        expected.updated_at_ns,
                        duration,
                    )
                    .map_err(verification)?
                    .work
            }
            (InsomniaWorkState::Processing, InsomniaWorkState::Terminal) => writer
                .session
                .insomnia
                .terminal(
                    &mut writer.container,
                    id,
                    prior
                        .lease_token
                        .ok_or_else(|| conflict("missing Insomnia lease"))?,
                    expected.updated_at_ns,
                    expected
                        .last_error
                        .clone()
                        .ok_or_else(|| conflict("missing terminal reason"))?,
                )
                .map_err(conflict)?,
            (InsomniaWorkState::Terminal, InsomniaWorkState::Pending) => writer
                .session
                .insomnia
                .retry_terminal(&mut writer.container, id, expected.updated_at_ns)
                .map_err(conflict)?,
            (InsomniaWorkState::Processing, InsomniaWorkState::Failed) => writer
                .session
                .insomnia
                .fail(
                    &mut writer.container,
                    id,
                    prior
                        .lease_token
                        .ok_or_else(|| conflict("missing Insomnia lease"))?,
                    expected.updated_at_ns,
                    expected
                        .retry_after_ns
                        .ok_or_else(|| conflict("missing retry time"))?,
                    expected
                        .last_error
                        .clone()
                        .ok_or_else(|| conflict("missing failure reason"))?,
                )
                .map_err(conflict)?,
            (InsomniaWorkState::Held, InsomniaWorkState::Pending) => writer
                .session
                .insomnia
                .retry_held(&mut writer.container, id, expected.updated_at_ns)
                .map_err(conflict)?,
            _ => {
                return Err(RelReconciliationError::UnsupportedState(
                    "import Insomnia transition lacks destination owner replay",
                ));
            }
        };
        writer.session.writer_lease_proof = writer.container.writer_lease_proof();
        if actual != *expected {
            return Err(conflict(
                "import Insomnia transition changed accepted journal revision",
            ));
        }
    }
    let final_work = writer
        .session
        .insomnia
        .work(id)
        .ok_or_else(|| conflict("replayed import has no final Insomnia claim"))?;
    if final_work.state != InsomniaWorkState::Processing {
        return Err(conflict(
            "import Insomnia journal has no active completion claim",
        ));
    }
    final_work
        .lease_token
        .ok_or_else(|| conflict("replayed import claim lacks lease"))
}
