//! Import-owned historical manifest policy over admitted owner facts.
use crate::freshness::{FreshnessPolicy, FreshnessRecord};
use crate::freshness_storage::FreshnessReplayEntry;
use crate::history::manifest_encoding::Value;
use crate::owner_lifecycle::OwnerError;
use crate::{MemoryId, freshness::historical_planner::MemoryAvailability};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

fn fail(message: &str) -> OwnerError {
    OwnerError::Freshness(message.into())
}

/// Admit the durable owner and policy for an epoch-scoped historical manifest.
pub(crate) fn validate_epoch_owner(
    owner: Option<[u8; 16]>,
    epoch_owner: [u8; 16],
) -> Result<[u8; 16], OwnerError> {
    let owner = owner.ok_or_else(|| fail("historical import requires a durable REL owner"))?;
    if owner != epoch_owner {
        return Err(fail("unsupported import owner or policy"));
    }
    Ok(owner)
}

pub(crate) fn validate_epoch_policy(policy: FreshnessPolicy) -> Result<(), OwnerError> {
    if policy != FreshnessPolicy::default_v1() {
        return Err(fail("unsupported import owner or policy"));
    }
    Ok(())
}

/// Shared legacy whole-owner preflight for the facade entry point and native manifest.
/// Inputs are already captured owner facts; this helper performs no reads or mutation.
pub(crate) fn validate_legacy_owner_policy(
    owner_uuid: Option<[u8; 16]>,
    activity_cut: u64,
    has_open_epoch: bool,
    policy: FreshnessPolicy,
    has_pending_dream_pass: bool,
    insomnia: crate::InsomniaStats,
) -> Result<[u8; 16], OwnerError> {
    let owner = owner_uuid.ok_or_else(|| fail("historical import requires a durable REL owner"))?;
    if (activity_cut == 0 && !has_open_epoch)
        || policy != FreshnessPolicy::default_v1()
        || has_pending_dream_pass
    {
        return Err(fail(
            "unsupported import owner/policy or unfinished generation",
        ));
    }
    if insomnia.pending + insomnia.processing + insomnia.failed + insomnia.terminal != 0 {
        return Err(fail("import Insomnia generation is not complete"));
    }
    Ok(owner)
}

/// Require settled publication-birth evidence for one already selected cohort member.
pub(crate) fn validate_settled_birth(
    is_publication_birth: bool,
    is_settled: bool,
) -> Result<(), OwnerError> {
    if !is_publication_birth || !is_settled {
        return Err(fail(
            "import cohort lacks settled first-cycle/publication evidence",
        ));
    }
    Ok(())
}

pub(crate) fn validate_supported_cohort(
    is_epoch: bool,
    initial_is_empty: bool,
) -> Result<(), OwnerError> {
    if initial_is_empty && !is_epoch {
        return Err(fail("import has no supported active cohort"));
    }
    Ok(())
}

pub(crate) fn historical_import_id(owner: [u8; 16], corpus: [u8; 32]) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(b"reliquary:historical-import:v1\0");
    hash.update([7]);
    hash.update(2u32.to_le_bytes());
    for (key, value) in [
        ("corpus_digest", corpus.as_slice()),
        ("owner_uuid", owner.as_slice()),
    ] {
        hash.update([5]);
        hash.update((key.len() as u32).to_le_bytes());
        hash.update(key.as_bytes());
        hash.update([4]);
        hash.update((value.len() as u32).to_le_bytes());
        hash.update(value);
    }
    hash.finalize().into()
}

pub(crate) fn decode_import_id(value: &str) -> [u8; 32] {
    let mut out = [0; 32];
    for (index, byte) in out.iter_mut().enumerate() {
        let pair = &value[index * 2..index * 2 + 2];
        *byte = u8::from_str_radix(pair, 16).expect("validated durable import ID");
    }
    out
}

pub(crate) struct ReplacementProof {
    pub(crate) value: Value,
    pub(crate) cohort: BTreeSet<MemoryId>,
}

/// Compose the replacement/isolation proof from already admitted Import and Freshness facts.
pub(crate) fn replacement_proof(
    epoch: bool,
    ids: &BTreeSet<MemoryId>,
    born_ids: &BTreeSet<MemoryId>,
    owner_ids: &BTreeSet<MemoryId>,
    initial: &BTreeMap<MemoryId, FreshnessRecord>,
    superseded: &BTreeSet<String>,
    affected_recipients: Option<&BTreeSet<MemoryId>>,
    replay_entries: &[FreshnessReplayEntry],
) -> Result<ReplacementProof, OwnerError> {
    let mut cohort = if epoch {
        born_ids.clone()
    } else {
        owner_ids.clone()
    };
    if let Some(affected) = affected_recipients {
        cohort.extend(affected.iter().copied());
    }

    let mut untouched = Vec::new();
    let base_recipient_records = if epoch {
        ids.difference(born_ids)
            .map(|id| {
                let record = initial.get(id).ok_or_else(|| {
                    fail("scoped pre-start recipient lacks its authenticated base record")
                })?;
                Ok(Value::map([
                    ("memory_id", Value::bytes(id.0)),
                    (
                        "base_record",
                        crate::history::community_proof::record_value(*record),
                    ),
                ]))
            })
            .collect::<Result<Vec<_>, OwnerError>>()?
    } else {
        for entry in replay_entries {
            if matches!(entry, FreshnessReplayEntry::AcceptedUseReceipt { .. }) {
                return Err(fail(
                    "legacy reconstruction cannot replace retained accepted use without an import base",
                ));
            }
            if let FreshnessReplayEntry::Event {
                event_id, effects, ..
            } = entry
                && !superseded.contains(event_id)
            {
                untouched.push(Value::map([
                    ("event_id", Value::text(event_id)),
                    (
                        "recipients",
                        Value::Array(effects.keys().map(|id| Value::bytes(id.0)).collect()),
                    ),
                ]));
            }
        }
        Vec::new()
    };
    let reconstructed_memories: BTreeSet<_> = if epoch {
        cohort.clone()
    } else {
        initial.keys().copied().collect()
    };
    let untouched_memory_ids: BTreeSet<_> = if epoch {
        ids.difference(&reconstructed_memories).copied().collect()
    } else {
        owner_ids
            .difference(&reconstructed_memories)
            .copied()
            .collect()
    };
    let mut value = Value::map([
        (
            "superseded_event_ids",
            Value::Array(superseded.iter().map(Value::text).collect()),
        ),
        (
            "untouched_events",
            crate::history::community_proof::sorted(untouched)
                .map_err(|error| fail(&error.to_string()))?,
        ),
        (
            if epoch {
                "born_and_provisionally_affected_memories"
            } else {
                "reconstructed_memories"
            },
            Value::Array(
                reconstructed_memories
                    .iter()
                    .map(|id| Value::bytes(id.0))
                    .collect(),
            ),
        ),
        (
            if epoch {
                "recipients_without_provisional_effects"
            } else {
                "untouched_memories"
            },
            Value::Array(
                untouched_memory_ids
                    .iter()
                    .map(|id| Value::bytes(id.0))
                    .collect(),
            ),
        ),
        (
            "isolation",
            Value::text("verified-disjoint-provisional-dream-effects-v1"),
        ),
    ]);
    if epoch {
        let Value::Map(fields) = &mut value else {
            unreachable!("historical replacement proof root is a map");
        };
        fields.push((
            "base_recipient_records".to_owned(),
            Value::Array(base_recipient_records),
        ));
    }
    Ok(ReplacementProof { value, cohort })
}

pub(crate) fn authorize_propagation(
    adjacency: crate::freshness::MemoryAdjacency,
    memberships: crate::freshness::CommunityMembership,
    memory_facts: &BTreeMap<MemoryId, (Option<u64>, bool)>,
) -> crate::imports::historical_settlement_plan::PropagationInputs {
    let availability = memory_facts
        .iter()
        .map(|(&id, &(available_at, credit_eligible))| {
            let value = match available_at {
                Some(position) if credit_eligible => MemoryAvailability::Supported { position },
                Some(_) => MemoryAvailability::CreditExcluded,
                None => MemoryAvailability::Unsupported,
            };
            (id, value)
        })
        .collect();
    crate::imports::historical_settlement_plan::PropagationInputs {
        adjacency,
        memberships,
        authorized: memory_facts.keys().copied().collect(),
        availability,
    }
}

pub(crate) fn embedding_pass(
    generation: Option<crate::VectorGeneration>,
    source_records: &[Value],
) -> Result<Value, OwnerError> {
    let generation = generation.ok_or_else(|| fail("completed fill lacks retained generation"))?;
    // Source entries include a derived coverage_digest; the fill digest commits
    // exactly fragment/bindings/availability/mode/evidence, without hashing it twice.
    let records = source_records
        .iter()
        .map(|record| match record {
            Value::Map(fields) => Ok(Value::Map(
                fields
                    .iter()
                    .filter(|(key, _)| key != "coverage_digest")
                    .cloned()
                    .collect(),
            )),
            _ => Err(fail("malformed embedding source coverage")),
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Value::map([
        ("contract_version", Value::Unsigned(2)),
        ("disposition", Value::text("complete")),
        (
            "profile_id",
            Value::bytes(generation.compatibility_profile_id.0),
        ),
        ("selected_generation_id", Value::bytes(generation.id.0)),
        (
            "source_archive_version",
            Value::Unsigned(generation.source_archive_version),
        ),
        (
            "coverage_digest",
            Value::bytes(
                Value::Array(records)
                    .digest("reliquary:historical-embedding-coverage:v2")
                    .map_err(OwnerError::from)?,
            ),
        ),
    ]))
}
