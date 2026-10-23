//! Finish accepted Community, Insomnia, Dream and Freshness replay state.
use super::OwnerError;
use crate::community_store::{CommunityOpenState, CommunityStore};
use crate::freshness_dream_journal::DreamFreshnessJournal;
use crate::freshness_storage::FreshnessStore;
use crate::graph_store::GraphStore;
use crate::insomnia::rebuild::InsomniaOpenState;
use crate::insomnia::store::InsomniaStore;
use crate::memory_store::MemoryStore;
use crate::{Archive, Container};

pub(crate) struct ReliquaryAcceptedReplay {
    pub(crate) communities: CommunityStore,
    pub(crate) insomnia: InsomniaStore,
}

/// Finish the ordered accepted-state replay following the Graph rewrite
/// checkpoint and preceding FreshnessDue construction.
#[allow(clippy::too_many_arguments)]
pub(crate) fn finish_reliquary_accepted_replay(
    container: &Container,
    community_state: CommunityOpenState,
    graph: &GraphStore,
    insomnia_state: InsomniaOpenState,
    archive: &Archive,
    memories: &MemoryStore,
    defer_dream_replay: bool,
    dream_freshness: &mut DreamFreshnessJournal,
    freshness: &mut FreshnessStore,
) -> Result<ReliquaryAcceptedReplay, OwnerError> {
    let communities = community_state.finish(graph, container.owner_uuid())?;
    let mut insomnia = insomnia_state.finish()?;
    super::insomnia_replay::validate_references(&insomnia, archive, memories)?;
    super::insomnia_replay::rebuild_schedule(&mut insomnia, archive)?;
    if defer_dream_replay {
        *dream_freshness = DreamFreshnessJournal::new(container.owner_uuid());
        super::dream_replay::replay_with_population(container, archive, memories, dream_freshness)?;
    }
    super::rebuild_freshness_from_replay(container, archive, memories, freshness, dream_freshness)?;
    Ok(ReliquaryAcceptedReplay {
        communities,
        insomnia,
    })
}
