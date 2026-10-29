//! Read-only classification of owner-authorized historical source facts.
//!
//! Source lookup, retention, lifecycle checks and domain errors stay with the
//! owning product facade. This crate only maps those supplied facts to the
//! shared historical result vocabulary.

pub use reliquary_archive::archive_error::ArchivePayloadUnavailable as PayloadUnavailable;

/// Payload availability is independent of lifecycle. A tombstone need not
/// destroy bytes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HistoricalPayload<T, S> {
    Available(T),
    Tombstoned(T),
    Pruned {
        structural_history: S,
        reason: PayloadUnavailable,
    },
}

/// Classify an owner read while preserving its error and source-read ordering.
///
/// `recognize_pruned` only recognizes the domain's specific pruned error.
/// Lifecycle is called only when the full payload read succeeded; retained
/// structure is called only for a recognized pruned error. Callers use the
/// latter callback to select the exact requested revision and produce their
/// own domain-specific missing-record error.
pub fn evaluate<T, S, E>(
    revision: Result<T, E>,
    recognize_pruned: impl FnOnce(&E) -> Option<PayloadUnavailable>,
    lifecycle: impl FnOnce() -> Result<bool, E>,
    retained_structure: impl FnOnce() -> Result<S, E>,
) -> Result<HistoricalPayload<T, S>, E> {
    match revision {
        Ok(payload) => Ok(if lifecycle()? {
            HistoricalPayload::Tombstoned(payload)
        } else {
            HistoricalPayload::Available(payload)
        }),
        Err(error) => match recognize_pruned(&error) {
            Some(reason) => Ok(HistoricalPayload::Pruned {
                structural_history: retained_structure()?,
                reason,
            }),
            None => Err(error),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn evaluates_active_tombstoned_and_pruned_facts() {
        assert_eq!(
            evaluate::<_, (), ()>(Ok("body"), |_| None, || Ok(false), || Ok(())),
            Ok(HistoricalPayload::Available("body"))
        );
        assert_eq!(
            evaluate::<_, (), ()>(Ok("body"), |_| None, || Ok(true), || Ok(())),
            Ok(HistoricalPayload::Tombstoned("body"))
        );
        assert_eq!(
            evaluate::<(), _, _>(
                Err("pruned"),
                |error| (*error == "pruned").then_some(PayloadUnavailable::Pruned),
                || panic!("pruned reads must not query lifecycle"),
                || Ok(7_u64),
            ),
            Ok(HistoricalPayload::Pruned {
                structural_history: 7,
                reason: PayloadUnavailable::Pruned,
            })
        );
    }

    #[test]
    fn propagates_wrong_errors_and_keeps_callbacks_lazy() {
        assert_eq!(
            evaluate::<(), (), _>(
                Err("missing"),
                |_| None,
                || panic!("wrong error"),
                || { panic!("wrong error") }
            ),
            Err("missing")
        );
        assert_eq!(
            evaluate::<(), (), _>(
                Ok(()),
                |_| None,
                || Err("lifecycle"),
                || { panic!("available payloads do not read retention") }
            ),
            Err("lifecycle")
        );
        assert_eq!(
            evaluate::<(), (), _>(
                Err("pruned"),
                |_| Some(PayloadUnavailable::Pruned),
                || panic!("pruned payloads do not query lifecycle"),
                || Err("missing structure"),
            ),
            Err("missing structure")
        );
    }
}
