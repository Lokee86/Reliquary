//! Value mechanics only: no owner identities, clocks, storage or publication.
//! Owners retain field order, wire tags, errors and validate-before-publish rules.

/// For an optional value, `Set(None)` clears while `Unchanged` inherits.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum FieldChange<T> {
    Unchanged,
    Set(T),
}

impl<T: Clone + Eq> FieldChange<T> {
    pub(crate) fn between(previous: &T, next: &T) -> Self {
        if previous == next {
            Self::Unchanged
        } else {
            Self::Set(next.clone())
        }
    }
}

/// Build one candidate field without mutating the accepted prior value.
pub(crate) fn apply_field<T: Clone>(previous: &T, change: &FieldChange<T>) -> T {
    match change {
        FieldChange::Unchanged => previous.clone(),
        FieldChange::Set(value) => value.clone(),
    }
}

pub(crate) fn mask_is_known(mask: u64, known_mask: u64) -> bool {
    mask & !known_mask == 0
}
