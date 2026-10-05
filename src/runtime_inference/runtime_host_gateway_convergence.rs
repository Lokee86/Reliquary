//! P4.7 binds durable P3 conversation facts to the mounted REL distribution gate.
//! Reads are bounded source snapshots; no model, provider, network, or delivery I/O runs here.
use super::events::{RelSequencer, RelSequencerGate};
use super::observation::{
    CheckpointContentIdentity, ConversationDurableDelta, GenerationCheckpointContent,
    GenerationCheckpointObservation, GenerationObservation, RelObservationSummary,
};
use super::streams::{
    ConversationStreamProvider, StreamKey, StreamSnapshot, StreamSourceSnapshot, StreamSubscription,
};
use super::{GatewayRuntime, InstanceRuntime, ReliquaryRuntimeHostError, operation};
use crate::multiplexing_journal::{ConversationId, RelOwnerId};
use sha2::{Digest, Sha256};
use std::sync::Arc;

#[path = "runtime_host_gateway_convergence_deltas.rs"]
mod deltas;
use deltas::{
    checkpoint_position_map, derive_broad_events, generation_map, receipt_facts_changed,
    validate_distribution_identity, validate_receipt_facts,
};

const MAX_PUBLISHED_FACTS: usize = 128;

/// The caller already holds the attachment/owner admission leases. This function
/// acquires the mounted REL gate before any source read and releases every source
/// lock before running the persistence closure.
impl GatewayRuntime {
    pub(super) fn with_conversation_distribution<T>(
        &self,
        owner_id: &str,
        epoch: u64,
        action: impl FnOnce() -> Result<T, ReliquaryRuntimeHostError>,
    ) -> Result<T, ReliquaryRuntimeHostError> {
        let sequencer = self.distribution_sequencer(owner_id, epoch)?;
        let mut gate = sequencer.gate(epoch)?;
        let before = match self.read_conversation_distribution_source(owner_id, epoch, None) {
            Ok(source) => source,
            Err(error) => {
                gate.invalidate();
                return Err(error);
            }
        };

        let before_positions = match self.read_conversation_checkpoint_positions(owner_id, epoch) {
            Ok(positions) => positions,
            Err(error) => {
                gate.invalidate();
                return Err(error);
            }
        };

        let before_receipts = match self.read_conversation_receipt_facts(owner_id, epoch) {
            Ok(facts) => facts,
            Err(error) => {
                gate.invalidate();
                return Err(error);
            }
        };

        let result = action();
        let after = match self.read_conversation_distribution_source(owner_id, epoch, None) {
            Ok(source) => source,
            Err(_) => {
                gate.invalidate();
                return match result {
                    Ok(value) => Ok(value),
                    Err(mutation_error) => Err(mutation_error),
                };
            }
        };

        let after_positions = match self.read_conversation_checkpoint_positions(owner_id, epoch) {
            Ok(positions) => positions,
            Err(_) => {
                gate.invalidate();
                return match result {
                    Ok(value) => Ok(value),
                    Err(mutation_error) => Err(mutation_error),
                };
            }
        };

        let after_receipts = match self.read_conversation_receipt_facts(owner_id, epoch) {
            Ok(facts) => facts,
            Err(_) => {
                gate.invalidate();
                return match result {
                    Ok(value) => Ok(value),
                    Err(mutation_error) => Err(mutation_error),
                };
            }
        };

        match result {
            Err(error) => {
                if before != after
                    || before_positions != after_positions
                    || receipt_facts_changed(&before_receipts, &after_receipts)
                {
                    gate.invalidate();
                }
                Err(error)
            }
            Ok(value) => {
                if before == after
                    && before_positions == after_positions
                    && !receipt_facts_changed(&before_receipts, &after_receipts)
                {
                    return Ok(value);
                }
                if let Err(error) = publish_distribution_changes(
                    self,
                    &mut gate,
                    owner_id,
                    epoch,
                    &before,
                    &after,
                    &before_receipts,
                    &after_receipts,
                    &before_positions,
                    &after_positions,
                ) {
                    // P3 durability is already authoritative. Distribution is
                    // recoverable only from a fresh snapshot after this gate closes.
                    gate.invalidate();
                    let _ = error;
                }
                Ok(value)
            }
        }
    }

    fn distribution_sequencer(
        &self,
        owner_id: &str,
        epoch: u64,
    ) -> Result<Arc<RelSequencer>, ReliquaryRuntimeHostError> {
        let rels = self
            .rel_executions
            .read()
            .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
        let rel = rels
            .get(owner_id)
            .ok_or_else(|| operation("conversation REL is not mounted"))?;
        if rel.mount_epoch.load(std::sync::atomic::Ordering::SeqCst) != epoch {
            return Err(operation(
                "conversation distribution belongs to a previous mount",
            ));
        }
        Ok(Arc::clone(&rel.sequencer))
    }

    pub(super) fn subscribe_authoritative_conversation_stream(
        &self,
        instance: &InstanceRuntime,
        key: StreamKey,
        capacity: usize,
    ) -> Result<(StreamSnapshot, StreamSubscription), ReliquaryRuntimeHostError> {
        let provider = AuthoritativeStreamProvider {
            gateway: self,
            instance,
        };
        self.join_conversation_stream(instance, key, capacity, &provider)
    }

    pub(super) fn reconnect_authoritative_conversation_stream(
        &self,
        instance: &InstanceRuntime,
        previous: &StreamSubscription,
        capacity: usize,
    ) -> Result<(StreamSnapshot, StreamSubscription), ReliquaryRuntimeHostError> {
        let provider = AuthoritativeStreamProvider {
            gateway: self,
            instance,
        };
        self.reconnect_conversation_stream(instance, previous, capacity, &provider)
    }
}

struct AuthoritativeStreamProvider<'a> {
    gateway: &'a GatewayRuntime,
    instance: &'a InstanceRuntime,
}
impl ConversationStreamProvider for AuthoritativeStreamProvider<'_> {
    fn snapshot(&self, key: &StreamKey) -> Result<StreamSourceSnapshot, ReliquaryRuntimeHostError> {
        self.gateway
            .read_conversation_stream_source(self.instance, key)
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
struct GenerationKey {
    conversation_id: ConversationId,
    operation_id: String,
    generation_id: String,
    branch_id: String,
}
impl GenerationKey {
    fn new(conversation_id: &ConversationId, generation: &GenerationObservation) -> Self {
        Self {
            conversation_id: conversation_id.clone(),
            operation_id: generation.identity.operation_id.clone(),
            generation_id: generation.identity.generation_id.clone(),
            branch_id: generation.identity.branch_id.clone(),
        }
    }
}

fn publish_distribution_changes(
    gateway: &GatewayRuntime,
    gate: &mut RelSequencerGate<'_>,
    owner_id: &str,
    epoch: u64,
    before: &RelObservationSummary,
    after: &RelObservationSummary,
    before_receipts: &[ConversationDurableDelta],
    after_receipts: &[ConversationDurableDelta],
    before_positions: &[GenerationCheckpointObservation],
    after_positions: &[GenerationCheckpointObservation],
) -> Result<(), ReliquaryRuntimeHostError> {
    validate_distribution_identity(owner_id, epoch, before)?;
    validate_distribution_identity(owner_id, epoch, after)?;
    validate_receipt_facts(owner_id, epoch, before_receipts)?;
    validate_receipt_facts(owner_id, epoch, after_receipts)?;
    let broad = derive_broad_events(before, after, before_receipts, after_receipts)?;
    let checkpoint_positions = checkpoint_position_map(owner_id, epoch, before_positions)?;
    let after_checkpoint_positions = checkpoint_position_map(owner_id, epoch, after_positions)?;
    let old_generations = generation_map(before);
    let new_generations = generation_map(after);
    for (key, position) in &checkpoint_positions {
        let generation = old_generations
            .get(key)
            .copied()
            .ok_or_else(|| operation("checkpoint position has no matching durable generation"))?;
        if generation.durable_checkpoint_sequence != Some(position.durable_checkpoint_sequence)
            || generation.status != position.status
        {
            return Err(operation(
                "checkpoint position does not match the before generation snapshot",
            ));
        }
    }
    for (key, generation) in &old_generations {
        if generation.durable_checkpoint_sequence.is_some()
            && !checkpoint_positions.contains_key(key)
        {
            return Err(operation("before checkpoint position is missing"));
        }
    }
    for (key, position) in &after_checkpoint_positions {
        let generation = new_generations.get(key).copied().ok_or_else(|| {
            operation("after checkpoint position has no matching durable generation")
        })?;
        if generation.durable_checkpoint_sequence != Some(position.durable_checkpoint_sequence)
            || generation.status != position.status
        {
            return Err(operation(
                "checkpoint position does not match the after generation snapshot",
            ));
        }
    }
    for (key, generation) in &new_generations {
        if generation.durable_checkpoint_sequence.is_some()
            && !after_checkpoint_positions.contains_key(key)
        {
            return Err(operation("after checkpoint position is missing"));
        }
    }
    for (key, before_position) in &checkpoint_positions {
        if let Some(after_position) = after_checkpoint_positions.get(key) {
            if before_position.durable_checkpoint_sequence
                == after_position.durable_checkpoint_sequence
                && before_position.content != after_position.content
            {
                return Err(operation(
                    "checkpoint bytes changed without a durable sequence advance",
                ));
            }
        }
    }
    let changed_generations: Vec<_> = new_generations
        .iter()
        .filter(|(key, generation)| old_generations.get(*key).copied() != Some(*generation))
        .map(|(key, generation)| (key.clone(), (**generation).clone()))
        .collect();

    // Source snapshots are bounded at 16,384 records. Gate fanout is smaller,
    // so refuse an oversized cut before reconstructing any checkpoint text.
    if broad.len() + changed_generations.len() > MAX_PUBLISHED_FACTS {
        return Err(operation(
            "conversation distribution change exceeds the per-cut event bound",
        ));
    }
    if old_generations
        .keys()
        .any(|key| !new_generations.contains_key(key))
    {
        return Err(operation(
            "durable generation disappeared; fresh snapshot required",
        ));
    }

    // StreamHub caps each mount epoch at 256 exact stream identities; overflow
    // invalidates this sequencer and requires a fresh snapshot after epoch reset.
    let mut stream_events = Vec::with_capacity(changed_generations.len());
    for (key, generation) in changed_generations {
        let rel_id = RelOwnerId::parse_canonical(owner_id).map_err(operation)?;
        let stream_key = StreamKey {
            rel_id,
            mount_epoch: epoch,
            conversation_id: key.conversation_id.clone(),
            operation_id: key.operation_id.clone(),
            generation_id: key.generation_id.clone(),
            branch_id: key.branch_id.clone(),
        };
        let previous = old_generations.get(&key).copied();
        let checkpoint = match generation.durable_checkpoint_sequence {
            Some(sequence)
                if previous.and_then(|old| old.durable_checkpoint_sequence) != Some(sequence) =>
            {
                let content = gateway
                    .read_conversation_checkpoint(
                        owner_id,
                        epoch,
                        &key.conversation_id,
                        &generation,
                    )?
                    .ok_or_else(|| operation("durable checkpoint content is unavailable"))?;
                Some(checkpoint_event(
                    owner_id,
                    epoch,
                    &key.conversation_id,
                    previous,
                    checkpoint_positions.get(&key).copied(),
                    &generation,
                    content,
                )?)
            }
            _ => None,
        };
        stream_events.push((stream_key, generation, checkpoint));
    }

    // Source readers above release all runtime/coordinator/journal locks before
    // notifications are appended. A partial fanout invalidates the whole gate.
    for event in broad {
        let watermarks = after.durable_watermarks.clone();
        if let Err(error) = gate.publish(event.class, watermarks, vec![event.invalidation]) {
            gate.invalidate();
            return Err(error);
        }
    }
    for (key, generation, checkpoint) in stream_events {
        if let Err(error) = gate.publish_stream(key, generation, checkpoint) {
            gate.invalidate();
            return Err(error);
        }
    }
    gate.observe_snapshot_watermarks(after.durable_watermarks.clone());
    Ok(())
}

fn checkpoint_event(
    owner_id: &str,
    epoch: u64,
    conversation_id: &ConversationId,
    previous: Option<&GenerationObservation>,
    previous_position: Option<&GenerationCheckpointObservation>,
    generation: &GenerationObservation,
    mut current: GenerationCheckpointContent,
) -> Result<GenerationCheckpointContent, ReliquaryRuntimeHostError> {
    let rel_id = RelOwnerId::parse_canonical(owner_id).map_err(operation)?;
    let sequence = generation
        .durable_checkpoint_sequence
        .ok_or_else(|| operation("checkpoint event has no durable sequence"))?;
    if current.observation.rel_id != rel_id
        || current.observation.mount_epoch != epoch
        || current.observation.conversation_id != *conversation_id
        || current.observation.generation != generation.identity
        || current.observation.durable_checkpoint_sequence != sequence
        || current.observation.status != generation.status
    {
        return Err(operation(
            "checkpoint source does not match durable generation",
        ));
    }
    let old_sequence = previous.and_then(|old| old.durable_checkpoint_sequence);
    if let Some(old_sequence) = old_sequence {
        let can_delta = previous_position.is_some_and(|position| {
            position.rel_id == rel_id
                && position.mount_epoch == epoch
                && position.conversation_id == *conversation_id
                && position.generation == generation.identity
                && position.durable_checkpoint_sequence == old_sequence
                && matches!(position.content, CheckpointContentIdentity::Prefix { .. })
        });
        if can_delta {
            let position = previous_position.expect("checked above");
            if let CheckpointContentIdentity::Prefix { utf8_bytes, sha256 } = &position.content {
                if let Ok(prefix_bytes) = usize::try_from(*utf8_bytes) {
                    if let (Some(prefix), Some(suffix)) = (
                        current.text.get(..prefix_bytes),
                        current.text.get(prefix_bytes..),
                    ) {
                        let actual: [u8; 32] = Sha256::digest(prefix.as_bytes()).into();
                        if &actual == sha256 {
                            let digest: [u8; 32] = Sha256::digest(suffix.as_bytes()).into();
                            current.text = suffix.to_owned();
                            current.observation.content = CheckpointContentIdentity::Delta {
                                after_checkpoint_sequence: old_sequence,
                                utf8_bytes: current.text.len() as u64,
                                sha256: digest,
                            };
                        }
                    }
                }
            }
        }
    } else {
        // A first checkpoint is a delta from the empty prefix. This also seeds
        // the stream hub so later exact deltas are continuity checked.
        let digest: [u8; 32] = Sha256::digest(current.text.as_bytes()).into();
        current.observation.content = CheckpointContentIdentity::Delta {
            after_checkpoint_sequence: 0,
            utf8_bytes: current.text.len() as u64,
            sha256: digest,
        };
    }
    Ok(current)
}
