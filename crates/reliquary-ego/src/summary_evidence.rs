//! Shared citation binding for authorized Ego summary packets.

use reliquary_memory_audit::{SourceCitation, SourceId};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::BTreeSet;

pub(crate) fn same_memory(a: &SourceCitation, b: &SourceCitation) -> bool {
    a.owner_id == b.owner_id && a.source == b.source && matches!(a.source, SourceId::Memory(_))
}

pub(crate) fn same_revision(a: &SourceCitation, b: &SourceCitation) -> bool {
    same_memory(a, b)
        && a.history_generation == b.history_generation
        && a.revision == b.revision
        && a.body_id == b.body_id
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MemoryEvidence {
    pub citation: SourceCitation,
    pub source_date: String,
    pub text: String,
}

impl MemoryEvidence {
    pub(crate) fn valid(&self) -> bool {
        matches!(self.citation.source, SourceId::Memory(_))
            && self.citation.start_byte < self.citation.end_byte
            && (self.citation.end_byte - self.citation.start_byte) as usize == self.text.len()
    }

    pub(crate) fn covers(&self, citation: &SourceCitation) -> bool {
        let passage = &self.citation;
        self.valid()
            && same_revision(passage, citation)
            && passage.field == citation.field
            && passage.start_byte <= citation.start_byte
            && citation.end_byte <= passage.end_byte
            && citation.start_byte < citation.end_byte
            && self
                .text
                .is_char_boundary((citation.start_byte - passage.start_byte) as usize)
            && self
                .text
                .is_char_boundary((citation.end_byte - passage.start_byte) as usize)
    }

    fn bind(&self, quote: &str) -> Option<SourceCitation> {
        if quote.trim().is_empty() {
            return None;
        }
        let start = self.text.find(quote)?;
        // Include overlapping occurrences and preserve byte offsets into the authorized range.
        let after = start + self.text[start..].chars().next()?.len_utf8();
        if self.text[after..].contains(quote) {
            return None;
        }
        let start_byte = self.citation.start_byte + u32::try_from(start).ok()?;
        let end_byte = start_byte + u32::try_from(quote.len()).ok()?;
        Some(SourceCitation {
            start_byte,
            end_byte,
            ..self.citation
        })
    }
}

pub struct StructuredRequest {
    pub instruction: String,
    pub input: Value,
    pub schema: Value,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Support {
    pub(crate) slot: usize,
    quote: String,
}

pub(crate) fn bind_support(
    evidence: &[MemoryEvidence],
    support: &[Support],
) -> Option<Vec<SourceCitation>> {
    if support.is_empty() {
        return None;
    }
    let mut seen = BTreeSet::new();
    let mut citations = Vec::new();
    for item in support {
        if !seen.insert((item.slot, &item.quote)) {
            return None;
        }
        let citation = evidence.get(item.slot)?.bind(&item.quote)?;
        if !citations.contains(&citation) {
            citations.push(citation);
        }
    }
    Some(citations)
}

pub(crate) fn object_schema(properties: Value) -> Value {
    let required: Vec<_> = properties
        .as_object()
        .expect("schema properties")
        .keys()
        .collect();
    json!({
        "type": "object",
        "properties": properties,
        "required": required,
        "additionalProperties": false
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Judgment {
    Supported,
    Unsupported,
    Uncertain,
}
