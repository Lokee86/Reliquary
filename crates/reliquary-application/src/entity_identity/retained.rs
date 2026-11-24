use reliquary_owner_session::{
    OwnerSessionError, RelSession,
    model::{
        MemoryError, MemoryId, MemoryTextField,
        identity::{IdentityCitation, IdentityReceipt, IdentitySide},
    },
};

/// A correction may supersede a judgment while both owners still retain the cited spans.
pub fn retained_identity_quotes(
    left: &RelSession,
    right: &RelSession,
    receipt: &IdentityReceipt,
) -> Result<Option<Vec<String>>, OwnerSessionError> {
    if left.owner_id() != receipt.pair.local_owner || right.owner_id() != receipt.pair.peer_owner {
        return Ok(None);
    }
    let mut quotes = Vec::with_capacity(receipt.citations.len());
    for citation in &receipt.citations {
        let (owner, refs, endpoint) = match citation.side() {
            IdentitySide::Left => (left, &receipt.left, receipt.pair.local_entity),
            IdentitySide::Right => (right, &receipt.right, receipt.pair.peer_entity),
        };
        let (relationship, memory) = match *citation {
            IdentityCitation::Relationship {
                relationship,
                memory,
                ..
            } => (relationship, memory),
            IdentityCitation::Mention { mention, .. } => {
                let Some(reference) = refs.mentions.get(usize::from(mention)) else {
                    return Ok(None);
                };
                let Some(quote) = owner.retained_identity_mention_quote(
                    endpoint,
                    refs.endpoint_revision,
                    reference,
                )?
                else {
                    return Ok(None);
                };
                quotes.push(quote);
                continue;
            }
        };
        let Some(relation) = refs.relationships.get(usize::from(relationship)) else {
            return Ok(None);
        };
        let Some(memory) = relation.memories.get(usize::from(memory)) else {
            return Ok(None);
        };
        let Some(span) = memory.span else {
            return Ok(None);
        };
        let Some(retained) = owner.relationship_revision(relation.id, relation.revision) else {
            return Ok(None);
        };
        if owner
            .relationship_description_prune(relation.id, relation.revision)
            .is_some()
        {
            return Ok(None);
        }
        let owner_hex: String = owner
            .owner_id()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect();
        if !retained
            .evidence
            .iter()
            .any(|r| r.owner_id == owner_hex && r.memory_id == memory.id)
        {
            return Ok(None);
        }
        let reader = owner.management_read();
        let (record, body) =
            match reader.memory_projection(MemoryId(memory.id), Some(memory.revision)) {
                Ok(value) => value,
                Err(MemoryError::MissingMemory | MemoryError::RevisionConflict) => return Ok(None),
                Err(e) => return Err(e.into()),
            };
        let Some(body) = body.filter(|_| record.body_id.0 == memory.body_id) else {
            return Ok(None);
        };
        let field = match span.field {
            MemoryTextField::Title => &body.title,
            MemoryTextField::Content => &body.content,
        };
        let Some(quote) = field.get(span.start_byte as usize..span.end_byte as usize) else {
            return Ok(None);
        };
        if quote != retained.summary {
            return Ok(None);
        }
        quotes.push(quote.to_owned());
    }
    Ok(Some(quotes))
}
