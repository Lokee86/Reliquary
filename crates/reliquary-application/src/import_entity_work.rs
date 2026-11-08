//! Recover exact mention inputs from accepted extraction checkpoints.
use crate::{EntityMemoryMention, NativeInsomniaExtraction, TextField};
use reliquary_insomnia::{RoutingMention, TextField as ExtractedField};
use reliquary_owner_session::{
    CompletedImportExtraction, OwnerSessionError, RelSession, model::MemoryId,
};

pub struct ImportEntityWork {
    pub mentions: Vec<EntityMemoryMention>,
    pub routed_memories: Vec<MemoryId>,
}

pub fn accepted_import_entity_work(
    owner: &RelSession,
    completed: CompletedImportExtraction,
    phy: Option<[u8; 16]>,
) -> Result<ImportEntityWork, OwnerSessionError> {
    let CompletedImportExtraction { claim, receipt } = completed;
    let error = |message: &str| OwnerSessionError::Record(message.into());
    let payload = claim
        .work
        .held_payload
        .as_deref()
        .ok_or_else(|| error("import Entity work requires an extraction checkpoint"))?;
    let extraction = NativeInsomniaExtraction::reopen_import_checkpoint(&claim, phy, payload)
        .map_err(|_| error("import Entity checkpoint does not match the paired owner"))?;
    if extraction.model() != receipt.extractor_model {
        return Err(error(
            "import Entity checkpoint does not match the accepted model",
        ));
    }
    let prepared = extraction
        .prepare(owner, &claim)
        .map_err(|_| error("cannot revalidate the accepted import Entity inputs"))?;
    if prepared.project_memories().count() != receipt.memory_ids.len()
        || prepared.user_memories().count() != receipt.external_memory_refs.len()
        || prepared.rejected().len() != receipt.rejected_count as usize
    {
        return Err(error(
            "import Entity checkpoint differs from its completion receipt",
        ));
    }
    let mut work = ImportEntityWork {
        mentions: Vec::new(),
        routed_memories: Vec::new(),
    };
    for (candidate, id) in prepared.project_memories().zip(receipt.memory_ids) {
        add_mentions(
            &mut work,
            owner.owner_id(),
            MemoryId(id.0),
            &candidate.routing_mentions,
        );
    }
    if let Some(phy) = phy {
        let expected_owner = phy
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        for (candidate, reference) in prepared.user_memories().zip(receipt.external_memory_refs) {
            if reference.owner_id != expected_owner {
                return Err(error("import Entity source routes to another PHY owner"));
            }
            let id = MemoryId(reference.memory_id.0);
            work.routed_memories.push(id);
            add_mentions(&mut work, phy, id, &candidate.routing_mentions);
        }
    }
    Ok(work)
}

fn add_mentions(
    work: &mut ImportEntityWork,
    owner_id: [u8; 16],
    memory_id: MemoryId,
    mentions: &[RoutingMention],
) {
    work.mentions
        .extend(mentions.iter().map(|mention| EntityMemoryMention {
            owner_id,
            memory_id,
            field: match mention.field {
                ExtractedField::Title => TextField::Title,
                ExtractedField::Content => TextField::Content,
            },
            start_byte: mention.start_byte,
            end_byte: mention.end_byte,
            text: mention.text.clone(),
        }));
}
