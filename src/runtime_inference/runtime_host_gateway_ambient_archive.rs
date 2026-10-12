use super::*;
use crate::{
    DEFAULT_ARCHIVE_SEARCH_RESULTS, MAX_ARCHIVE_SEARCH_QUERY_BYTES, MAX_ARCHIVE_SEARCH_RESULTS,
    RuntimeSemanticOwner, RuntimeSemanticOwnerKind, VisibleArchiveSearchOwner,
    VisibleArchiveSearchResult,
};

impl GatewayRuntime {
    /// Search all retained branches of only those RELs in this instance's
    /// explicitly granted dependency closure. Each result stays owner-tagged.
    pub fn search_visible_archive(
        &self,
        instance: &InstanceRuntime,
        query: &str,
        limit: usize,
    ) -> Result<VisibleArchiveSearchResult, ReliquaryRuntimeHostError> {
        let (query, limit) = validate_branch_archive_query(query, limit)?;
        let authorization = instance.authorization().map_err(operation)?;
        let owners = self.visible_semantic_owners(instance)?;
        let mut results = Vec::new();
        for owner in owners
            .into_iter()
            .filter(|owner| owner.kind == RuntimeSemanticOwnerKind::Reliquary)
        {
            results.push(self.branch_archive_search_for_owner(instance, owner, query, limit));
        }
        self.with_instance_delivery(instance, authorization, || {
            Ok(VisibleArchiveSearchResult { owners: results })
        })
    }

    pub fn search_archive_owner(
        &self,
        instance: &InstanceRuntime,
        owner_id: &str,
        query: &str,
        limit: usize,
    ) -> Result<VisibleArchiveSearchOwner, ReliquaryRuntimeHostError> {
        let (query, limit) = validate_branch_archive_query(query, limit)?;
        let token = self.authorize(instance, owner_id, OwnerGrant::ViewRel)?;
        // Public owner-specific searches need explicit grant but not membership
        // in the current selected REL's dependency closure.
        let owner = RuntimeSemanticOwner {
            owner_id: owner_id.to_owned(),
            kind: RuntimeSemanticOwnerKind::Reliquary,
        };
        let result = self.branch_archive_search_for_owner(instance, owner, query, limit);
        self.deliver_rel(instance, &token, result)
    }

    fn branch_archive_search_for_owner(
        &self,
        instance: &InstanceRuntime,
        owner: RuntimeSemanticOwner,
        query: &str,
        limit: usize,
    ) -> VisibleArchiveSearchOwner {
        let hits = self.branch_archive_hits_for(instance, &owner.owner_id, query, limit);
        match hits {
            Ok(hits) => VisibleArchiveSearchOwner {
                owner,
                hits,
                error: None,
            },
            Err(error) => VisibleArchiveSearchOwner {
                owner,
                hits: Vec::new(),
                error: Some(error.to_string()),
            },
        }
    }

    fn branch_archive_hits_for(
        &self,
        instance: &InstanceRuntime,
        owner_id: &str,
        query: &str,
        limit: usize,
    ) -> Result<Vec<ConversationSearchHit>, ReliquaryRuntimeHostError> {
        let mut unique = HashMap::new();
        for summary in self.conversation_summaries_for(instance, owner_id)? {
            for leaf in summary.leaf_node_ids {
                for hit in self.search_conversation_branch_for(
                    instance,
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
        hits.sort_by(|a, b| {
            b.score
                .total_cmp(&a.score)
                .then_with(|| a.fragment.conversation_id.cmp(&b.fragment.conversation_id))
                .then_with(|| a.fragment.start_node_id.cmp(&b.fragment.start_node_id))
                .then_with(|| a.fragment.end_node_id.cmp(&b.fragment.end_node_id))
        });
        hits.truncate(limit);
        Ok(hits)
    }
}

fn validate_branch_archive_query(
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
