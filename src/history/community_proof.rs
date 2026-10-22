//! Canonical historical record and Community proof values.
use crate::freshness::FreshnessRecord;
use crate::{COMMUNITY_ALGORITHM_VERSION, CommunitySnapshot, MemoryId};
use std::collections::BTreeSet;

use super::manifest_encoding::{HistoryProofError, Value};

pub(crate) fn record_value(record: FreshnessRecord) -> Value {
    Value::map([
        ("score", Value::Signed(i64::from(record.score))),
        (
            "admitted_at_turn",
            record.admitted_at_turn.map_or(Value::Null, Value::Unsigned),
        ),
        (
            "accounted_turn",
            record.accounted_turn.map_or(Value::Null, Value::Unsigned),
        ),
    ])
}

pub(crate) fn sorted(values: Vec<Value>) -> Result<Value, HistoryProofError> {
    let mut keyed = values
        .into_iter()
        .map(|value| value.encoded().map(|key| (key, value)))
        .collect::<Result<Vec<_>, _>>()?;
    keyed.sort_by(|a, b| a.0.cmp(&b.0));
    if keyed.windows(2).any(|pair| pair[0].0 == pair[1].0) {
        return Err(HistoryProofError::new(
            "duplicate historical proof identity",
        ));
    }
    Ok(Value::Array(
        keyed.into_iter().map(|(_, value)| value).collect(),
    ))
}

pub(crate) fn community_proof(
    snapshot: Option<CommunitySnapshot>,
    graph_version: u64,
    ids: &BTreeSet<MemoryId>,
) -> Result<Value, HistoryProofError> {
    let Some(mut snapshot) = snapshot else {
        return Ok(Value::map([("mode", Value::text("unknown-outside-v1"))]));
    };
    // Malformed present evidence rejects even when the version is stale.
    if (!snapshot.resolution.is_finite() || snapshot.resolution <= 0.0)
        || !snapshot.quality.is_finite()
        || snapshot.generation == 0
        || snapshot.algorithm_version != COMMUNITY_ALGORITHM_VERSION
    {
        return Err(HistoryProofError::new(
            "malformed historical Community parameters",
        ));
    }
    snapshot.communities.sort_by_key(|community| community.id);
    let mut communities = Vec::new();
    let mut seen = BTreeSet::new();
    let mut names = BTreeSet::new();
    for mut community in snapshot.communities {
        community.members.sort();
        if community.members.is_empty()
            || !names.insert(community.id)
            || community
                .members
                .iter()
                .any(|id| !ids.contains(id) || !seen.insert(*id))
        {
            return Err(HistoryProofError::new(
                "malformed, duplicate or foreign historical Community membership",
            ));
        }
        communities.push(Value::map([
            ("community_id", Value::bytes(community.id.0)),
            (
                "members",
                Value::Array(
                    community
                        .members
                        .into_iter()
                        .map(|id| Value::bytes(id.0))
                        .collect(),
                ),
            ),
        ]));
    }
    let membership = Value::Array(communities);
    let evidence = Value::map([
        ("generation", Value::Unsigned(snapshot.generation)),
        (
            "derived_graph_version",
            Value::Unsigned(snapshot.derived_graph_version),
        ),
        (
            "algorithm_version",
            Value::Unsigned(u64::from(snapshot.algorithm_version)),
        ),
        ("seed", Value::Unsigned(snapshot.seed)),
        (
            "resolution",
            Value::bytes(snapshot.resolution.to_le_bytes()),
        ),
        ("quality", Value::bytes(snapshot.quality.to_le_bytes())),
        ("membership", membership.clone()),
        (
            "membership_digest",
            Value::bytes(membership.digest("reliquary:historical-community-membership:v1")?),
        ),
    ]);
    Ok(Value::map([
        (
            "mode",
            Value::text(if snapshot.derived_graph_version == graph_version {
                "validated-v1"
            } else {
                "unknown-outside-v1"
            }),
        ),
        (
            "membership",
            if snapshot.derived_graph_version == graph_version {
                membership
            } else {
                Value::Array(vec![])
            },
        ),
        ("snapshot", evidence),
    ]))
}
