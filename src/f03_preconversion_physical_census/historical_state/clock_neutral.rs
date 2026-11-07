//! Historical commitments for retained clock-neutral Community and resolution records.
use super::{ToolResult, lower_hex, sha256_bytes};
use crate::community_codec::{decode_semantic_name, decode_snapshot};
use crate::community_lineage::derive_lineage;
use crate::entity_resolution_codec::{comparison_carrier, retained};
use crate::entity_resolution_store::EntityResolutionStore;
use crate::historical_fingerprint::{HistoricalFingerprint, OwnerProjection};
use crate::{CommunitySemanticNameSource, CommunitySnapshot, Container};
use serde_json::json;

pub(super) fn observe(container: &Container) -> ToolResult<HistoricalFingerprint> {
    let uuid = container
        .owner_uuid()
        .ok_or("clock-neutral history needs owner UUID")?;
    let mut evidence = OwnerProjection::new(uuid, "clock_neutral_history", 0, None)?;
    let mut cursor = container
        .physical_cursor()
        .map_err(|_| "clock-neutral history cursor failed")?;
    let mut previous: Option<CommunitySnapshot> = None;
    let mut resolutions = EntityResolutionStore::default();
    let mut resolution_ordinal = 0_u64;
    let mut names_ordinal = 0_u64;
    let mut inert_ordinal = 0_u64;

    while let Some(frame) = cursor
        .next_frame()
        .map_err(|_| "clock-neutral frame rejected")?
    {
        let payload = &frame.payload;
        if let Some(snapshot) =
            decode_snapshot(payload).map_err(|_| "Community snapshot rejected")?
        {
            evidence.insert("community_snapshot", snapshot.generation.to_be_bytes(), json!({
                "generation": snapshot.generation,
                "derived_graph_version": snapshot.derived_graph_version,
                "algorithm_version": snapshot.algorithm_version,
                "seed": snapshot.seed,
                "resolution_f64_le": lower_hex(&snapshot.resolution.to_bits().to_le_bytes()),
                "quality_f64_le": lower_hex(&snapshot.quality.to_bits().to_le_bytes()),
                "communities": snapshot.communities.iter().map(|community| json!({
                    "id": lower_hex(&community.id.0),
                    "members": community.members.iter().map(|id| lower_hex(&id.0)).collect::<Vec<_>>(),
                })).collect::<Vec<_>>(),
            }))?;
            if let Some(old) = &previous {
                let transition = derive_lineage(old, &snapshot);
                evidence.insert("community_lineage", snapshot.generation.to_be_bytes(), json!({
                    "from_generation": transition.from_generation,
                    "to_generation": transition.to_generation,
                    "links": transition.links.iter().map(|link| json!({
                        "predecessor": lower_hex(&link.predecessor.0),
                        "successor": lower_hex(&link.successor.0),
                        "intersection": link.intersection,
                        "predecessor_members": link.predecessor_members,
                        "successor_members": link.successor_members,
                        "continuation": link.continuation,
                        "split": link.split,
                        "merge": link.merge,
                    })).collect::<Vec<_>>(),
                    "new": transition.new_communities.iter().map(|id| lower_hex(&id.0)).collect::<Vec<_>>(),
                    "ended": transition.ended_communities.iter().map(|id| lower_hex(&id.0)).collect::<Vec<_>>(),
                }))?;
            }
            previous = Some(snapshot);
        }
        if let Some(name) = decode_semantic_name(payload).map_err(|_| "Community name rejected")? {
            let source = match name.source {
                CommunitySemanticNameSource::Dream => "dream",
                CommunitySemanticNameSource::User => "user",
            };
            evidence.insert("community_name", names_ordinal.to_be_bytes(), json!({
                "community_id": lower_hex(&name.community_id.0),
                "baseline_community_id": lower_hex(&name.baseline_community_id.0),
                "contract_version": name.contract_version,
                "source": source,
                "name": name.name,
                "representative_memories": name.representative_memories.iter().map(|id| lower_hex(&id.0)).collect::<Vec<_>>(),
            }))?;
            names_ordinal += 1;
        }
        if let Some(bytes) =
            retained::decode(payload).map_err(|_| "inert resolution evidence rejected")?
        {
            evidence.insert(
                "retained_inert_resolution",
                inert_ordinal.to_be_bytes(),
                json!({
                    "availability": "inert",
                    "retained_codec": "CVAERS02",
                    "retained_payload_sha256": sha256_bytes(bytes),
                    "retained_payload_byte_length": bytes.len(),
                }),
            )?;
            inert_ordinal += 1;
        } else if let Some(change) = resolutions
            .decode(payload)
            .map_err(|_| "resolution delta rejected")?
        {
            let canonical =
                comparison_carrier(&change).map_err(|_| "resolution semantic comparison failed")?;
            evidence.insert(
                "entity_resolution_revision",
                resolution_ordinal.to_be_bytes(),
                json!({
                    "availability": if change.status.is_some() { "retained" } else { "tombstoned" },
                    "memory_id": lower_hex(&change.key.memory_id.0),
                    "field": change.key.field.tag(),
                    "start_byte": change.key.start_byte,
                    "end_byte": change.key.end_byte,
                    "revision": change.revision,
                    "updated_at_ns": change.updated_at_ns,
                    "comparison_sha256": sha256_bytes(&canonical),
                }),
            )?;
            resolutions
                .ingest(payload)
                .map_err(|_| "resolution history admission failed")?;
            resolution_ordinal += 1;
        }
    }
    evidence.fingerprint()
}
