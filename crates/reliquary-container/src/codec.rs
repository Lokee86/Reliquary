//! Value mechanics only: no owner identities, clocks, storage or publication.
//! Owners retain field order, wire tags, errors and validate-before-publish rules.

/// For an optional value, `Set(None)` clears while `Unchanged` inherits.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FieldChange<T> {
    Unchanged,
    Set(T),
}

impl<T: Clone + Eq> FieldChange<T> {
    pub fn between(previous: &T, next: &T) -> Self {
        if previous == next {
            Self::Unchanged
        } else {
            Self::Set(next.clone())
        }
    }
}

/// Build one candidate field without mutating the accepted prior value.
pub fn apply_field<T: Clone>(previous: &T, change: &FieldChange<T>) -> T {
    match change {
        FieldChange::Unchanged => previous.clone(),
        FieldChange::Set(value) => value.clone(),
    }
}

pub fn mask_is_known(mask: u64, known_mask: u64) -> bool {
    mask & !known_mask == 0
}
