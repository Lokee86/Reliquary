use super::{
    ReliquaryRuntimeHost, ReliquaryRuntimeHostError, operation,
    semantic_access::{RuntimeSemanticOwner, RuntimeSemanticOwnerKind},
};
use crate::{
    ArchiveSearchHit, ConversationSearchHit, MAX_ARCHIVE_SEARCH_QUERY_BYTES,
    MAX_ARCHIVE_SEARCH_RESULTS,
};
use crate::{VisibleArchiveSearchOwner, VisibleArchiveSearchResult};
use std::collections::HashMap;

pub const DEFAULT_ARCHIVE_SEARCH_RESULTS: usize = 5;

impl ReliquaryRuntimeHost {
    pub fn search_archive(
        &self,
        query: &str,
        limit: usize,
    ) -> Result<Vec<ArchiveSearchHit>, ReliquaryRuntimeHostError> {
        let owner_id = self.active_rel_id().ok_or_else(|| {
            ReliquaryRuntimeHostError::Operation("Reliquary runtime has no active REL".into())
        })?;
        self.search_archive_for(&owner_id, query, limit)
    }

    pub fn search_archive_for(
        &self,
        owner_id: &str,
        query: &str,
        limit: usize,
    ) -> Result<Vec<ArchiveSearchHit>, ReliquaryRuntimeHostError> {
        let owner = self.semantic_owner(owner_id)?;
        if owner.kind != RuntimeSemanticOwnerKind::Reliquary {
            return Err(operation("Archive search requires a Reliquary owner"));
        }
        self.with_runtime_for(owner_id, |runtime| {
            runtime.cva.search_archive(query, limit).map_err(operation)
        })
    }

    pub fn search_visible_archive(
        &self,
        query: &str,
        limit: usize,
    ) -> Result<VisibleArchiveSearchResult, ReliquaryRuntimeHostError> {
        let (query, limit) = validated_visible_archive_search(query, limit)?;
        let owners = self
            .visible_semantic_owners()?
            .into_iter()
            .filter(|owner| owner.kind == RuntimeSemanticOwnerKind::Reliquary)
            .collect::<Vec<_>>();
        std::thread::scope(|scope| {
            let workers = owners
                .into_iter()
                .map(|owner| {
                    let search_owner = owner.clone();
                    (
                        owner,
                        scope.spawn(move || {
                            self.search_all_branches_for(&search_owner.owner_id, query, limit)
                        }),
                    )
                })
                .collect::<Vec<_>>();
            let mut results = Vec::with_capacity(workers.len());
            for (owner, worker) in workers {
                match worker.join() {
                    Ok(Ok(hits)) => results.push(VisibleArchiveSearchOwner {
                        owner,
                        hits,
                        error: None,
                    }),
                    Ok(Err(error)) => results.push(VisibleArchiveSearchOwner {
                        owner,
                        hits: Vec::new(),
                        error: Some(error.to_string()),
                    }),
                    Err(_) => results.push(VisibleArchiveSearchOwner {
                        owner,
                        hits: Vec::new(),
                        error: Some("archive search worker panicked".into()),
                    }),
                }
            }
            Ok(VisibleArchiveSearchResult { owners: results })
        })
    }

    pub fn search_archive_owner(
        &self,
        owner_id: &str,
        query: &str,
        limit: usize,
    ) -> Result<VisibleArchiveSearchOwner, ReliquaryRuntimeHostError> {
        let (query, limit) = validated_visible_archive_search(query, limit)?;
        let owner = self.semantic_owner(owner_id)?;
        if owner.kind != RuntimeSemanticOwnerKind::Reliquary {
            return Err(operation("Archive search requires a Reliquary owner"));
        }
        self.search_archive_owner_inner(owner, query, limit)
    }

    pub fn search_visible_archive_owner(
        &self,
        owner_id: &str,
        query: &str,
        limit: usize,
    ) -> Result<VisibleArchiveSearchOwner, ReliquaryRuntimeHostError> {
        let (query, limit) = validated_visible_archive_search(query, limit)?;
        let owner = self.visible_semantic_owner(owner_id)?;
        if owner.kind != RuntimeSemanticOwnerKind::Reliquary {
            return Err(operation("Archive search requires a Reliquary owner"));
        }
        self.search_archive_owner_inner(owner, query, limit)
    }

    fn search_archive_owner_inner(
        &self,
        owner: RuntimeSemanticOwner,
        query: &str,
        limit: usize,
    ) -> Result<VisibleArchiveSearchOwner, ReliquaryRuntimeHostError> {
        match self.search_all_branches_for(&owner.owner_id, query, limit) {
            Ok(hits) => Ok(VisibleArchiveSearchOwner {
                owner,
                hits,
                error: None,
            }),
            Err(error) => Ok(VisibleArchiveSearchOwner {
                owner,
                hits: Vec::new(),
                error: Some(error.to_string()),
            }),
        }
    }

    fn search_all_branches_for(
        &self,
        owner_id: &str,
        query: &str,
        limit: usize,
    ) -> Result<Vec<ConversationSearchHit>, ReliquaryRuntimeHostError> {
        let summaries = self.conversation_summaries_for(owner_id)?;
        let mut unique = HashMap::new();
        for summary in summaries {
            for leaf in summary.leaf_node_ids {
                for hit in self.search_conversation_branch_for(
                    owner_id,
                    &summary.conversation_id,
                    &leaf,
                    query,
                    limit,
                )? {
                    let key = (
                        hit.fragment.conversation_id.clone(),
                        hit.fragment.start_node_id.clone(),
                        hit.fragment.end_node_id.clone(),
                    );
                    unique.entry(key).or_insert(hit);
                }
            }
        }
        let mut hits = unique.into_values().collect::<Vec<_>>();
        hits.sort_by(|left, right| {
            right
                .score
                .total_cmp(&left.score)
                .then_with(|| {
                    left.fragment
                        .conversation_id
                        .cmp(&right.fragment.conversation_id)
                })
                .then_with(|| {
                    left.fragment
                        .start_node_id
                        .cmp(&right.fragment.start_node_id)
                })
                .then_with(|| left.fragment.end_node_id.cmp(&right.fragment.end_node_id))
        });
        hits.truncate(limit);
        Ok(hits)
    }
}

fn validated_visible_archive_search(
    query: &str,
    limit: usize,
) -> Result<(&str, usize), ReliquaryRuntimeHostError> {
    let query = query.trim();
    if query.is_empty() || query.len() > MAX_ARCHIVE_SEARCH_QUERY_BYTES {
        return Err(operation(format!(
            "archive search requires a query of at most {MAX_ARCHIVE_SEARCH_QUERY_BYTES} bytes"
        )));
    }
    let limit = if limit == 0 {
        DEFAULT_ARCHIVE_SEARCH_RESULTS
    } else {
        limit
    };
    if !(1..=MAX_ARCHIVE_SEARCH_RESULTS).contains(&limit) {
        return Err(operation(format!(
            "archive search limit must be between 1 and {MAX_ARCHIVE_SEARCH_RESULTS}"
        )));
    }
    Ok((query, limit))
}
