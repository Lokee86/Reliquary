//! Independent bounded one-hop retrieval for each supplied anchor.

use super::*;
use reliquary_owner_session::model::NodeRef;
use reliquary_retrieval::agentic::MAX_AGENTIC_CANDIDATES;

pub(super) fn evaluate(
    endpoint: &dyn DecisionEndpoint,
    owner: [u8; 16],
    turn: &RetrievalTurn,
    next: &RetrievalTurn,
    relations: &[Relation],
    anchors: &[MemoryId],
    by_id: BTreeMap<MemoryId, &AgenticMemory>,
) -> Result<RetrievalEvaluation, String> {
    struct PreparedAnchor {
        report: RetrievalAnchorReport,
        offered: Vec<MemoryId>,
    }

    let mut prepared = Vec::with_capacity(anchors.len());
    for anchor in anchors {
        let mut evidence = BTreeSet::new();
        for edge in relations.iter().filter(|edge| edge.active) {
            let (NodeRef::Memory(source), NodeRef::Memory(target)) = (edge.source, edge.target)
            else {
                continue;
            };
            let (source, target) = (MemoryId(source), MemoryId(target));
            let (neighbor, direction) = if source == *anchor {
                (target, RetrievalDirection::Outgoing)
            } else if target == *anchor {
                (source, RetrievalDirection::Incoming)
            } else {
                continue;
            };
            if neighbor != *anchor && by_id.contains_key(&neighbor) {
                evidence.insert(RetrievalRelationEvidence {
                    neighbor,
                    kind: edge.kind,
                    direction,
                });
            }
        }

        let neighbors = evidence
            .iter()
            .map(|edge| edge.neighbor)
            .collect::<BTreeSet<_>>();
        let declined_neighbors = neighbors
            .iter()
            .filter(|id| turn.searched().contains(id) && !turn.selected().contains(id))
            .count();
        let unseen = neighbors
            .into_iter()
            .filter(|id| !turn.searched().contains(id) && !turn.selected().contains(id))
            .collect::<Vec<_>>();
        let unseen_neighbors = unseen.len();
        let mut ranked = unseen
            .into_iter()
            .map(|id| (id, by_id[&id].freshness))
            .collect::<Vec<_>>();
        ranked.sort_by(|(a_id, a), (b_id, b)| b.cmp(a).then_with(|| a_id.cmp(b_id)));
        ranked.truncate(MAX_AGENTIC_CANDIDATES);
        let offered = ranked.iter().map(|(id, _)| *id).collect::<Vec<_>>();
        let report = RetrievalAnchorReport {
            anchor: *anchor,
            unseen_neighbors,
            declined_neighbors,
            relations: evidence.into_iter().collect(),
            approved: BTreeSet::new(),
        };
        prepared.push(PreparedAnchor { report, offered });
    }

    // Each anchor owns an independent one-hop evaluation. Gather all workers
    // in deterministic anchor order before constructing any state transition.
    let results = std::thread::scope(|scope| {
        let by_id = &by_id;
        let workers = prepared
            .into_iter()
            .map(|mut anchor| {
                let query = next.query().unwrap().to_owned();
                scope.spawn(move || {
                    let approved = if anchor.offered.is_empty() {
                        Ok(BTreeSet::new())
                    } else {
                        super::decide(
                            endpoint,
                            owner,
                            "traverse_graph",
                            &query,
                            &anchor.offered,
                            &by_id,
                        )
                    }?;
                    anchor.report.approved = approved;
                    Ok((anchor.report, anchor.offered))
                })
            })
            .collect::<Vec<_>>();
        workers
            .into_iter()
            .map(|worker| {
                worker
                    .join()
                    .unwrap_or_else(|_| Err("traversal anchor worker panicked".into()))
            })
            .collect::<Vec<Result<_, String>>>()
    });
    let mut completed = Vec::with_capacity(results.len());
    for result in results {
        completed.push(result?);
    }
    let mut evaluated_union = BTreeSet::new();
    let mut approved_union = BTreeSet::new();
    let mut reports = Vec::with_capacity(completed.len());
    for (report, offered) in completed {
        evaluated_union.extend(offered);
        approved_union.extend(report.approved.iter().copied());
        reports.push(report);
    }
    let mut staged = next.clone();
    staged.record_evaluated(evaluated_union);
    Ok(RetrievalEvaluation {
        next: staged,
        memories: approved_union
            .into_iter()
            .filter_map(|id| by_id.get(&id).map(|memory| (*memory).clone()))
            .collect(),
        already_provided: BTreeSet::new(),
        anchors: reports,
    })
}
