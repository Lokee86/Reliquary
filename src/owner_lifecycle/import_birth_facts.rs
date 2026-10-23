//! Concrete accepted external publisher facts for historical Import base proof.
use super::HistorySource;
use crate::MemoryId;
use std::collections::BTreeMap;
/// The bounded external birth evidence History needs; the external publisher's
/// envelope and storage representation remain owned by its source adapter.
pub(crate) struct ExternalBirthFacts {
    pub(crate) birth_turn: Option<u64>,
    pub(crate) records: Vec<(MemoryId, u64, String)>,
}

/// Decode the allowed external-source publisher evidence at the facade edge.
/// History receives only bounded birth facts, never the external codec or its
/// envelope. Decode errors are kept at frame ends and replayed in scan order.
pub(crate) fn external_birth_facts(
    source: &HistorySource<'_>,
    prefix_end: u64,
) -> BTreeMap<u64, Result<ExternalBirthFacts, crate::MemoryError>> {
    let mut facts = BTreeMap::new();
    let Ok(mut cursor) = source.prefix_cursor(prefix_end) else {
        // The same stable borrowed source is rescanned by History, which emits
        // this physical/source failure in its original place in replay order.
        return facts;
    };
    loop {
        let frame = match cursor.next_frame() {
            Ok(Some(frame)) => frame,
            Ok(None) | Err(_) => {
                // Do not report a pre-scan failure here. History repeats this
                // bounded scan and preserves ordering with earlier evidence.
                break;
            }
        };
        match crate::memory_external_publication::decode(&frame.payload) {
            Ok(Some(publication)) => {
                facts.insert(
                    frame.end,
                    Ok(ExternalBirthFacts {
                        birth_turn: publication.birth_turn,
                        records: publication
                            .records
                            .into_iter()
                            .map(|record| (record.id, record.global_version, record.mutation_id))
                            .collect(),
                    }),
                );
            }
            Ok(None) => {}
            Err(error) => {
                facts.insert(frame.end, Err(error));
            }
        }
    }
    facts
}
