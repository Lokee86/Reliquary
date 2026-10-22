//! Test adapter for mapping a global ordering point to owner-local cuts.

pub(crate) use crate::history::cuts::GlobalOwnerCuts;
use crate::{Cva, Phylactery, history::cuts::OwnerCutFacts};

pub(crate) fn reliquary_global_cuts(cva: &Cva, global: u64) -> Result<GlobalOwnerCuts, String> {
    if global > cva.latest_global_version() {
        return Err("global cut exceeds retained owner head".into());
    }
    let history =
        crate::historical_graph_fingerprint::GraphHistory::read(&cva.container, &cva.graph)?;
    reliquary_global_cuts_with_positions(cva, global, &history.memory_graph_positions())
}

/// Same owner cut query with immutable Graph membership read once by a fixture oracle.
pub(crate) fn reliquary_global_cuts_with_positions(
    cva: &Cva,
    global: u64,
    memory_graph_positions: &[u64],
) -> Result<GlobalOwnerCuts, String> {
    crate::history::cuts::owner_cuts(
        cva.latest_global_version(),
        global,
        OwnerCutFacts {
            archive: Some(&cva.archive),
            memories: &cva.memories,
            entities: &cva.entities,
            relationships: &cva.relationships,
            graph: &cva.graph,
            vector_generations: Some(&cva.vector_generations),
            memory_graph_positions,
        },
    )
}

pub(crate) fn phylactery_global_cuts(
    phy: &Phylactery,
    global: u64,
) -> Result<GlobalOwnerCuts, String> {
    if global > phy.latest_global_version() {
        return Err("global cut exceeds retained owner head".into());
    }
    let history =
        crate::historical_graph_fingerprint::GraphHistory::read(&phy.container, &phy.graph)?;
    phylactery_global_cuts_with_positions(phy, global, &history.memory_graph_positions())
}

/// Same owner cut query with immutable Graph membership read once by a fixture oracle.
pub(crate) fn phylactery_global_cuts_with_positions(
    phy: &Phylactery,
    global: u64,
    memory_graph_positions: &[u64],
) -> Result<GlobalOwnerCuts, String> {
    crate::history::cuts::owner_cuts(
        phy.latest_global_version(),
        global,
        OwnerCutFacts {
            archive: None,
            memories: &phy.memories,
            entities: &phy.entities,
            relationships: &phy.relationships,
            graph: &phy.graph,
            vector_generations: None,
            memory_graph_positions,
        },
    )
}
