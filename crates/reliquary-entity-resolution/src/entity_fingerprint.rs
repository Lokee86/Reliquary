use crate::{EntityCandidateSet, Memory};
use sha2::{Digest, Sha256};

pub fn candidate_fingerprint(set: &EntityCandidateSet) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(b"reliquary-entity-candidate-set-v2\0");
    for candidate in &set.candidates {
        hash.update(candidate.entity.id.0);
        hash.update(candidate.entity.revision.to_le_bytes());
        hash.update([u8::from(candidate.exact_surface)]);
        hash.update([u8::from(candidate.normalized_surface)]);
        hash.update([u8::from(candidate.alias_surface)]);
        hash.update([u8::from(candidate.source_association)]);
        hash.update(candidate.lexical_memory_hits.to_le_bytes());
        hash.update(candidate.graph_neighbor_hits.to_le_bytes());
        hash.update(candidate.best_lexical_score.to_bits().to_le_bytes());
        for memory_id in &candidate.supporting_memory_ids {
            hash.update(memory_id.0);
        }
    }
    hash.finalize().into()
}

pub fn context_fingerprint(
    memory: &Memory,
    set: &EntityCandidateSet,
    evidence: &[Vec<Memory>],
    admission_context: &[Memory],
) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(b"reliquary-entity-resolution-context-v1\0");
    hash.update(memory.id.0);
    hash.update(set.key.start_byte.to_le_bytes());
    hash.update(set.key.end_byte.to_le_bytes());
    hash.update(set.mention.text.as_bytes());
    for memories in evidence {
        for value in memories {
            hash.update(value.id.0);
            hash.update(value.revision.to_le_bytes());
        }
    }
    for value in admission_context {
        hash.update(value.id.0);
        hash.update(value.revision.to_le_bytes());
    }
    hash.finalize().into()
}
