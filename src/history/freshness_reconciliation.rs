//! Read-only normalization of Freshness journal frames for owner replay.

use crate::community_store::CommunityStore;
use crate::freshness_dream_journal::DreamFreshnessJournal;
use crate::freshness_storage::{FreshnessReplayEntry, FreshnessStore};
use crate::graph_store::GraphStore;
use crate::{Archive, CvaReconcileError};
use std::collections::BTreeMap;

pub(crate) struct FreshnessReplaySource<'a> {
    pub(crate) owner_uuid: Option<[u8; 16]>,
    pub(crate) archive: &'a Archive,
    pub(crate) freshness: &'a FreshnessStore,
    pub(crate) dream_freshness: &'a DreamFreshnessJournal,
    pub(crate) graph: &'a GraphStore,
    pub(crate) communities: &'a CommunityStore,
}

pub(crate) struct RebasedFreshnessEntry {
    pub(crate) entry: FreshnessReplayEntry,
    pub(crate) destination_turn: u64,
}

pub(crate) fn normalize_replay_payload(
    source: &FreshnessReplaySource<'_>,
    destination_archive: &Archive,
    payload: &[u8],
) -> Result<Vec<RebasedFreshnessEntry>, CvaReconcileError> {
    let source_turns = activity_turns(source.archive);
    // Reconciliation's planned merged activity cut is a dense sequence over
    // the destination archive order, even when that archive retained sparse
    // physical activity positions. Keep this mapping aligned with the merge
    // planner's left-first identity ordering.
    let destination_turns = activity_turns(destination_archive)
        .into_values()
        .enumerate()
        .map(|(index, identity)| {
            (
                identity,
                u64::try_from(index).unwrap_or(u64::MAX).saturating_add(1),
            )
        })
        .collect::<BTreeMap<_, _>>();

    if payload.starts_with(crate::freshness_storage::REWRITE_BASELINE_MAGIC) {
        let baseline = source
            .freshness
            .rewrite_baselines
            .values()
            .find(|baseline| baseline.encode().ok().as_deref() == Some(payload))
            .ok_or(CvaReconcileError::UnsupportedSemanticOwner(
                "unvalidated rewrite baseline",
            ))?
            .clone();
        let destination_turn =
            rebase_turn(baseline.destination_cut, &source_turns, &destination_turns)?;
        if baseline.destination_cut != destination_turn {
            return Err(CvaReconcileError::UnsupportedSemanticOwner(
                "rewrite baseline activity prefix changed",
            ));
        }
        return Ok(vec![RebasedFreshnessEntry {
            entry: FreshnessReplayEntry::RewriteBaseline { baseline },
            destination_turn,
        }]);
    }

    let mut rebased = Vec::new();
    for mut entry in FreshnessReplayEntry::from_payload(payload)
        .map_err(crate::owner_lifecycle::OwnerError::Freshness)?
    {
        validate_semantic_source(source, &entry)?;
        if let FreshnessReplayEntry::Event {
            event_id, proof, ..
        } = &entry
        {
            ensure_event_proof_preserved(source, event_id, *proof)?;
        }
        let destination_turn =
            rebase_entry_turn(source, &entry, &source_turns, &destination_turns)?;
        match &mut entry {
            FreshnessReplayEntry::RewriteBaseline { .. } => unreachable!("baseline handled above"),
            FreshnessReplayEntry::Initialize { source_turn, .. }
            | FreshnessReplayEntry::Admit { source_turn, .. }
            | FreshnessReplayEntry::AcceptedUseReceipt { source_turn, .. } => {
                *source_turn = destination_turn;
            }
            FreshnessReplayEntry::Event {
                source_turn,
                provenance,
                semantic,
                ..
            } => {
                *source_turn = destination_turn;
                if let Some(provenance) = provenance.as_mut() {
                    provenance.accepted_turn = destination_turn;
                }
                if let Some(evidence) = semantic.as_mut() {
                    evidence.accepted_turn = destination_turn;
                }
            }
        }
        rebased.push(RebasedFreshnessEntry {
            entry,
            destination_turn,
        });
    }
    Ok(rebased)
}

pub(crate) fn ensure_event_proof_preserved(
    source: &FreshnessReplaySource<'_>,
    event_id: &str,
    proof: crate::FreshnessEventProof,
) -> Result<(), CvaReconcileError> {
    let accepted_use = source
        .freshness
        .event_has_accepted_provenance(event_id)
        .map_err(|_| {
            CvaReconcileError::UnsupportedSemanticOwner(
                "invalid accepted Freshness source provenance",
            )
        })?;
    let proof_available = proof.graph_version <= source.graph.memory_graph_version()
        && match (proof.community_generation, proof.community_graph_version) {
            (Some(generation), Some(version)) => {
                version == proof.graph_version
                    && source.communities.contains_snapshot(generation, version)
            }
            (None, None) => true,
            _ => false,
        };
    if accepted_use || source.dream_freshness.is_settled(event_id) || proof_available {
        return Ok(());
    }
    Err(CvaReconcileError::UnsupportedSemanticOwner(
        "Freshness event source graph/community proof is unavailable",
    ))
}

pub(crate) fn validate_semantic_source(
    source: &FreshnessReplaySource<'_>,
    entry: &FreshnessReplayEntry,
) -> Result<(), CvaReconcileError> {
    if let FreshnessReplayEntry::Event {
        event_id,
        source_turn,
        provenance,
        semantic,
        ..
    } = entry
    {
        if let Some(evidence) = semantic {
            let owner = source
                .owner_uuid
                .ok_or(CvaReconcileError::UnsupportedSemanticOwner(
                    "semantic replay lacks owner",
                ))?;
            let expected = evidence.event_id();
            if event_id != &expected
                || evidence.owner_uuid != owner
                || evidence.accepted_turn != *source_turn
                || semantic_evidence_position(source.archive, evidence).ok() != Some(*source_turn)
                || provenance.as_ref().is_none_or(|p| {
                    !crate::freshness_storage::is_semantic_producer(p.producer_kind)
                })
            {
                return Err(CvaReconcileError::UnsupportedSemanticOwner(
                    "semantic Freshness source identity is unavailable or conflicting",
                ));
            }
        } else if provenance
            .as_ref()
            .is_some_and(|p| crate::freshness_storage::is_semantic_producer(p.producer_kind))
        {
            return Err(CvaReconcileError::UnsupportedSemanticOwner(
                "semantic Freshness event lacks typed evidence",
            ));
        }
    }
    Ok(())
}

fn semantic_evidence_position(
    archive: &Archive,
    evidence: &crate::freshness_storage::SemanticEventEvidence,
) -> Result<u64, crate::ArchiveError> {
    if let Some(fragment) = &evidence.fragment {
        let record = archive.fragments.get(fragment.fragment_id).ok_or(
            crate::ArchiveError::CorruptRecord("missing semantic fragment"),
        )?;
        let mut sources: Vec<_> = archive
            .fragment_nodes_for(record)?
            .into_iter()
            .map(|node| (node.conversation_id, node.id))
            .collect();
        sources.sort();
        if sources != fragment.sources {
            return Err(crate::ArchiveError::CorruptRecord(
                "conflicting semantic fragment sources",
            ));
        }
        sources
            .iter()
            .filter_map(|(conversation, node)| {
                archive.activity_position_for_turn(conversation, node)
            })
            .max()
            .ok_or(crate::ArchiveError::CorruptRecord(
                "fragment has no accepted activity",
            ))
    } else {
        archive
            .activity_position_for_turn(&evidence.conversation_id, &evidence.node_id)
            .ok_or(crate::ArchiveError::CorruptRecord(
                "missing semantic source",
            ))
    }
}

fn activity_turns(archive: &Archive) -> BTreeMap<u64, (String, String)> {
    archive
        .nodes
        .iter()
        .filter_map(|node| {
            archive
                .activity_position_for_turn(&node.conversation_id, &node.id)
                .map(|turn| (turn, (node.conversation_id.clone(), node.id.clone())))
        })
        .collect()
}

fn rebase_entry_turn(
    source: &FreshnessReplaySource<'_>,
    entry: &FreshnessReplayEntry,
    turns: &BTreeMap<u64, (String, String)>,
    merged: &BTreeMap<(String, String), u64>,
) -> Result<u64, CvaReconcileError> {
    if let FreshnessReplayEntry::Event {
        semantic: Some(e), ..
    } = entry
    {
        if let Some(fragment) = &e.fragment {
            return fragment
                .sources
                .iter()
                .filter(|(conversation, node)| {
                    source
                        .archive
                        .activity_position_for_turn(conversation, node)
                        .is_some()
                })
                .map(|key| {
                    merged
                        .get(key)
                        .copied()
                        .ok_or(CvaReconcileError::UnsupportedSemanticOwner(
                            "missing fragment source mapping",
                        ))
                })
                .collect::<Result<Vec<_>, _>>()?
                .into_iter()
                .max()
                .ok_or(CvaReconcileError::UnsupportedSemanticOwner(
                    "empty fragment source mapping",
                ));
        }
    }
    rebase_turn(entry.source_turn(), turns, merged)
}

fn rebase_turn(
    source_turn: u64,
    source: &BTreeMap<u64, (String, String)>,
    merged: &BTreeMap<(String, String), u64>,
) -> Result<u64, CvaReconcileError> {
    if source_turn == 0 {
        return Ok(0);
    }
    let identity = source
        .get(&source_turn)
        .ok_or(CvaReconcileError::UnsupportedSemanticOwner(
            "Freshness source cut has no accepted Archive activity identity",
        ))?;
    merged
        .get(identity)
        .copied()
        .ok_or(CvaReconcileError::UnsupportedSemanticOwner(
            "Freshness source activity is absent from merged Archive order",
        ))
}
