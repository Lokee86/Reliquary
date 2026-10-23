//! Canonical, read-only vector coverage proof construction.

use crate::history::manifest_encoding::{HistoryProofError, Value};
use crate::owner_lifecycle::OwnerError;
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;

#[derive(Clone, Debug)]
pub(crate) struct VectorFragmentReadiness {
    pub(crate) complete: bool,
    pub(crate) source_digest: [u8; 32],
    pub(crate) generation_id: [u8; 32],
    pub(crate) profile_id: [u8; 32],
    pub(crate) invalid_vectors: u64,
    pub(crate) rows: Vec<VectorRowFacts>,
}

#[derive(Clone, Debug)]
pub(crate) struct VectorRowFacts {
    pub(crate) fragment_id: [u8; 32],
    pub(crate) row: u64,
    pub(crate) component_bytes: Vec<u8>,
}

#[derive(Clone, Debug)]
pub(crate) struct VectorNodeKey {
    pub(crate) conversation_id: String,
    pub(crate) node_id: String,
}

#[derive(Clone, Debug)]
pub(crate) struct VectorTargetBinding {
    pub(crate) memory_id: [u8; 32],
    pub(crate) body_id: [u8; 32],
    pub(crate) profile_id: [u8; 32],
    pub(crate) binding_id: Option<[u8; 32]>,
    pub(crate) packed_id: Option<[u8; 32]>,
    pub(crate) row: Option<u64>,
    pub(crate) row_bytes: Option<Vec<u8>>,
    pub(crate) usable: bool,
}

pub(crate) struct VectorProofBuilder {
    sources: Vec<Value>,
    fragments: Vec<Value>,
    profiles: Vec<Value>,
    targets: Vec<Value>,
    unavailable: u64,
    invalid: u64,
    mixed: u64,
}

impl VectorProofBuilder {
    pub(crate) fn new() -> Self {
        Self {
            sources: Vec::new(),
            fragments: Vec::new(),
            profiles: Vec::new(),
            targets: Vec::new(),
            unavailable: 0,
            invalid: 0,
            mixed: 0,
        }
    }

    pub(crate) fn include_source(
        key: &VectorNodeKey,
        position: u64,
        scoped_nodes: Option<&BTreeSet<(String, String)>>,
        scoped_base_cut: Option<u64>,
    ) -> bool {
        !scoped_nodes.is_some_and(|nodes| {
            !nodes.contains(&(key.conversation_id.clone(), key.node_id.clone()))
        }) && !scoped_base_cut.is_some_and(|base_cut| position <= base_cut)
    }

    pub(crate) fn record_source(&mut self, node: Value, position: u64) {
        self.sources.push(Value::map([
            ("node", node),
            ("position", Value::Unsigned(position)),
        ]));
    }

    pub(crate) fn fragment_scope(
        nodes: &[VectorNodeKey],
        scoped_nodes: Option<&BTreeSet<(String, String)>>,
    ) -> Result<bool, OwnerError> {
        let Some(scope) = scoped_nodes else {
            return Ok(true);
        };
        let overlaps = nodes
            .iter()
            .any(|node| scope.contains(&(node.conversation_id.clone(), node.node_id.clone())));
        let fully_scoped = !nodes.is_empty()
            && nodes
                .iter()
                .all(|node| scope.contains(&(node.conversation_id.clone(), node.node_id.clone())));
        if overlaps && !fully_scoped {
            return Err(OwnerError::Freshness(
                "vector fragment mixes import and external source nodes".into(),
            ));
        }
        Ok(fully_scoped)
    }

    pub(crate) fn fragment_after_base(position: Option<u64>, base_cut: Option<u64>) -> bool {
        !base_cut.is_some_and(|cut| position.is_some_and(|at| at <= cut))
    }

    pub(crate) fn record_fragment(
        &mut self,
        fragment_id: [u8; 32],
        conversation_id: &str,
        start_node_id: &str,
        end_node_id: &str,
        represented_nodes: Vec<Value>,
        position: Option<u64>,
        activity_nodes: usize,
        readiness: Option<VectorFragmentReadiness>,
    ) -> Result<(), OwnerError> {
        self.mixed += u64::from(activity_nodes > 1);
        let (mode, evidence) = match readiness {
            Some(readiness) if readiness.complete => {
                self.invalid += readiness.invalid_vectors;
                let rows = readiness
                    .rows
                    .into_iter()
                    .map(|row| {
                        Value::map([
                            ("fragment_id", Value::bytes(row.fragment_id)),
                            ("row", Value::Unsigned(row.row)),
                            (
                                "row_digest",
                                Value::bytes(Sha256::digest(row.component_bytes)),
                            ),
                        ])
                    })
                    .collect();
                (
                    "complete",
                    Value::map([
                        ("source_digest", Value::bytes(readiness.source_digest)),
                        ("generation_id", Value::bytes(readiness.generation_id)),
                        ("profile_id", Value::bytes(readiness.profile_id)),
                        (
                            "invalid_vectors",
                            Value::Unsigned(readiness.invalid_vectors),
                        ),
                        ("usable_rows", Value::Array(rows)),
                    ]),
                )
            }
            _ => {
                self.unavailable += u64::from(position.is_some());
                (
                    if position.is_some() {
                        "unavailable"
                    } else {
                        "non-activity-only"
                    },
                    Value::map([(
                        "reason",
                        Value::text("missing-or-incomplete-fragment-evidence"),
                    )]),
                )
            }
        };
        let evidence_digest = evidence
            .digest("reliquary:historical-fragment-coverage:v1")
            .map_err(proof_error)?;
        self.fragments.push(Value::map([
            ("fragment_id", Value::bytes(fragment_id)),
            ("conversation_id", Value::text(conversation_id)),
            ("start_node_id", Value::text(start_node_id)),
            ("end_node_id", Value::text(end_node_id)),
            ("represented_nodes", Value::Array(represented_nodes)),
            (
                "availability",
                position.map_or(Value::Null, Value::Unsigned),
            ),
            ("mode", Value::text(mode)),
            ("evidence", evidence),
            ("coverage_digest", Value::bytes(evidence_digest)),
        ]));
        Ok(())
    }

    pub(crate) fn record_profile(&mut self, profile_id: [u8; 32], descriptor: Vec<u8>) {
        let descriptor_digest = Sha256::digest(&descriptor);
        self.profiles.push(Value::map([
            ("profile_id", Value::bytes(profile_id)),
            ("descriptor", Value::bytes(descriptor)),
            ("descriptor_digest", Value::bytes(descriptor_digest)),
        ]));
    }

    pub(crate) fn record_target(&mut self, binding: VectorTargetBinding) {
        let mut fields = vec![
            ("memory_id".to_owned(), Value::bytes(binding.memory_id)),
            ("body_id".to_owned(), Value::bytes(binding.body_id)),
            ("profile_id".to_owned(), Value::bytes(binding.profile_id)),
        ];
        match (
            binding.binding_id,
            binding.packed_id,
            binding.row,
            binding.row_bytes,
        ) {
            (Some(binding_id), Some(packed_id), Some(row), Some(row_bytes)) => {
                fields.extend([
                    ("binding_id".to_owned(), Value::bytes(binding_id)),
                    ("packed_id".to_owned(), Value::bytes(packed_id)),
                    ("row".to_owned(), Value::Unsigned(row)),
                    (
                        "row_digest".to_owned(),
                        Value::bytes(Sha256::digest(row_bytes)),
                    ),
                    (
                        "status".to_owned(),
                        Value::text(if binding.usable {
                            "usable"
                        } else {
                            "excluded-invalid-or-incompatible"
                        }),
                    ),
                ]);
            }
            _ => fields.push(("status".to_owned(), Value::text("missing-binding"))),
        }
        self.targets.push(Value::Map(fields));
    }

    pub(crate) fn fragments_for_embedding(&self) -> &[Value] {
        &self.fragments
    }

    pub(crate) fn finish(
        self,
        generation: Option<([u8; 32], [u8; 32], [u8; 32], u64, u64)>,
        embedding_pass: Value,
    ) -> (Value, Vec<Value>) {
        let generations = generation.map_or_else(
            || Value::Array(vec![]),
            |(id, profile_id, archive_vector_id, source_archive_version, vector_version)| {
                Value::Array(vec![Value::map([
                    ("generation_id", Value::bytes(id)),
                    ("profile_id", Value::bytes(profile_id)),
                    ("archive_vector_id", Value::bytes(archive_vector_id)),
                    (
                        "source_archive_version",
                        Value::Unsigned(source_archive_version),
                    ),
                    ("vector_version", Value::Unsigned(vector_version)),
                ])])
            },
        );
        let source_count = self.sources.len() as u64;
        let fragment_count = self.fragments.len() as u64;
        (
            Value::map([
                ("profiles", Value::Array(self.profiles)),
                ("selected_generations", generations),
                ("target_bindings", Value::Array(self.targets)),
                ("fragment_sources", Value::Array(self.fragments)),
                (
                    "coverage",
                    Value::map([
                        ("activity_sources", Value::Unsigned(source_count)),
                        ("fragments", Value::Unsigned(fragment_count)),
                        ("unavailable_fragments", Value::Unsigned(self.unavailable)),
                        ("invalid_source_rows", Value::Unsigned(self.invalid)),
                        ("mixed_source_fragments", Value::Unsigned(self.mixed)),
                    ]),
                ),
                ("embedding_pass", embedding_pass),
            ]),
            self.sources,
        )
    }
}

fn proof_error(error: HistoryProofError) -> OwnerError {
    OwnerError::Freshness(error.to_string())
}

fn vector_admission_failure(message: &str) -> OwnerError {
    OwnerError::Freshness(message.into())
}
pub(crate) struct PreparedVectorProof {
    proof: VectorProofBuilder,
    generation: Option<([u8; 32], [u8; 32], [u8; 32], u64, u64)>,
}
impl PreparedVectorProof {
    pub(crate) fn fragments_for_embedding(&self) -> &[Value] {
        self.proof.fragments_for_embedding()
    }
    pub(crate) fn finish(self, embedding_pass: Value) -> (Value, Vec<Value>) {
        self.proof.finish(self.generation, embedding_pass)
    }
}
/// Read accepted source and target vector evidence before any provider-fill proof.
pub(crate) fn read_vector_admission(
    archive: &crate::Archive,
    memories: &crate::memory_store::MemoryStore,
    vectors: &crate::owner_lifecycle::semantic_source::HistoricalVectorReadAccess<'_>,
    order: &[crate::freshness::historical_planner::SourceKey],
    target_ids: &BTreeSet<crate::MemoryId>,
    scoped_nodes: Option<&BTreeSet<(String, String)>>,
    scoped_base_cut: Option<u64>,
) -> Result<PreparedVectorProof, OwnerError> {
    use crate::history::vector_proof::{
        VectorFragmentReadiness, VectorNodeKey, VectorProofBuilder, VectorRowFacts,
        VectorTargetBinding,
    };
    use crate::owner_lifecycle::semantic_source::SemanticSourceReadiness;

    let generation = vectors.generation();
    let mut proof = VectorProofBuilder::new();
    for (index, key) in order.iter().enumerate() {
        let position = index as u64 + 1;
        let key_facts = VectorNodeKey {
            conversation_id: key.conversation_id.clone(),
            node_id: key.node_id.clone(),
        };
        if !VectorProofBuilder::include_source(&key_facts, position, scoped_nodes, scoped_base_cut)
        {
            continue;
        }
        let node = archive
            .nodes
            .get(&key.conversation_id, &key.node_id)
            .ok_or_else(|| vector_admission_failure("missing historical source"))?;
        proof.record_source(crate::history::source_proof::node_binding(node), position);
    }

    for fragment in archive.fragments() {
        let nodes = archive.fragment_nodes_for(&fragment)?;
        let node_keys: Vec<_> = nodes
            .iter()
            .map(|node| VectorNodeKey {
                conversation_id: node.conversation_id.clone(),
                node_id: node.id.clone(),
            })
            .collect();
        if !VectorProofBuilder::fragment_scope(&node_keys, scoped_nodes)? {
            continue;
        }
        let position =
            crate::owner_lifecycle::semantic_source::fragment_position(archive, fragment.id)?;
        if !VectorProofBuilder::fragment_after_base(position, scoped_base_cut) {
            continue;
        }
        let activity_nodes = nodes
            .iter()
            .filter(|n| {
                archive
                    .activity_position_for_turn(&n.conversation_id, &n.id)
                    .is_some()
            })
            .count();
        let readiness = generation
            .map(|g| vectors.fragment(fragment.id, g.id))
            .transpose()?
            .map(|readiness| match readiness {
                SemanticSourceReadiness::Ready(source) => VectorFragmentReadiness {
                    complete: true,
                    source_digest: source.source_digest,
                    generation_id: source.generation.id.0,
                    profile_id: source.generation.compatibility_profile_id.0,
                    invalid_vectors: source.invalid_vectors as u64,
                    rows: source
                        .vectors
                        .into_iter()
                        .map(|vector| VectorRowFacts {
                            fragment_id: vector.fragment_id.0,
                            row: vector.row,
                            component_bytes: vector
                                .components
                                .iter()
                                .flat_map(|component| component.to_le_bytes())
                                .collect(),
                        })
                        .collect(),
                },
                SemanticSourceReadiness::Pending => VectorFragmentReadiness {
                    complete: false,
                    source_digest: [0; 32],
                    generation_id: [0; 32],
                    profile_id: [0; 32],
                    invalid_vectors: 0,
                    rows: Vec::new(),
                },
            });
        proof.record_fragment(
            fragment.id.0,
            &fragment.conversation_id,
            &fragment.start_node_id,
            &fragment.end_node_id,
            nodes
                .iter()
                .map(crate::history::source_proof::node_binding)
                .collect(),
            position,
            activity_nodes,
            readiness,
        )?;
    }

    for profile in vectors.profiles() {
        let descriptor = crate::compatibility_profile_codec::encode_profile(&profile)
            .map_err(|e| vector_admission_failure(&e.to_string()))?;
        proof.record_profile(profile.id.0, descriptor);
    }

    let mut ids: Vec<_> = target_ids.iter().copied().collect();
    ids.sort();
    for id in ids {
        let body = memories.current_body_id(id)?;
        for profile in vectors.profiles() {
            let Some(location) = vectors.target_location(profile.id, body) else {
                proof.record_target(VectorTargetBinding {
                    memory_id: id.0,
                    body_id: body.0,
                    profile_id: profile.id.0,
                    binding_id: None,
                    packed_id: None,
                    row: None,
                    row_bytes: None,
                    usable: false,
                });
                continue;
            };
            let set = vectors
                .target_set(location.set_id)
                .map_err(|e| vector_admission_failure(&e.to_string()))?;
            if set.compatibility_profile_id != profile.id
                || set.memory_body_ids.get(location.row as usize) != Some(&body)
            {
                return Err(vector_admission_failure(
                    "historical target vector binding conflict",
                ));
            }
            let packed = vectors.packed(set.packed_vector_id)?;
            let row = packed
                .row(location.row as usize)
                .ok_or_else(|| vector_admission_failure("missing historical target row"))?;
            let compatible = packed.schema().dimensions == profile.dimensions
                && packed.schema().scalar == lodestone_packed::ScalarType::F32;
            let components: Vec<_> = row
                .chunks_exact(4)
                .map(|b| f32::from_le_bytes(b.try_into().unwrap()))
                .collect();
            let usable = compatible
                && crate::freshness::semantic_policy::cosine_similarity(&components, &components)
                    .is_ok();
            proof.record_target(VectorTargetBinding {
                memory_id: id.0,
                body_id: body.0,
                profile_id: profile.id.0,
                binding_id: Some(set.id.0),
                packed_id: Some(set.packed_vector_id.0),
                row: Some(location.row),
                row_bytes: Some(row.to_vec()),
                usable,
            });
        }
    }

    let generation_facts = generation.map(|g| {
        (
            g.id.0,
            g.compatibility_profile_id.0,
            g.archive_vector_id.0,
            g.source_archive_version,
            g.vector_version,
        )
    });
    Ok(PreparedVectorProof {
        proof,
        generation: generation_facts,
    })
}
