//! Pure candidate-selection policies shared by REL and PHY rewrite planning.

use reliquary_memory_core::MemoryId;
use std::collections::BTreeSet;

pub type RewriteCandidate = (MemoryId, bool, String, Vec<u8>);

/// Apply explicit include/exclude choices to owner-authorized Memory candidates.
/// Exclusion takes precedence when an ID appears in both request lists.
pub fn apply_explicit_selection(
    candidates: &mut [RewriteCandidate],
    include_ids: &[String],
    exclude_ids: &[String],
) -> Result<(), String> {
    let included = include_ids
        .iter()
        .map(|s| parse_id(s, "Memory ID"))
        .collect::<Result<BTreeSet<_>, _>>()?;
    let excluded = exclude_ids
        .iter()
        .map(|s| parse_id(s, "Memory ID"))
        .collect::<Result<BTreeSet<_>, _>>()?;
    let all: BTreeSet<_> = candidates.iter().map(|(id, ..)| id.0).collect();
    if let Some(id) = included.iter().find(|id| !all.contains(*id)) {
        return Err(format!(
            "explicitly included Memory {} does not exist in the source",
            id_hex(id)
        ));
    }
    if let Some(id) = excluded.iter().find(|id| !all.contains(*id)) {
        return Err(format!(
            "explicitly excluded Memory {} does not exist in the source",
            id_hex(id)
        ));
    }
    for (id, retain, reason, _) in candidates {
        if excluded.contains(&id.0) {
            *retain = false;
            *reason = "explicitly excluded".into();
        } else if included.contains(&id.0) {
            *retain = true;
            *reason = "explicitly included".into();
        }
    }
    Ok(())
}

/// Add semantic evidence dependencies to the retained population. A dependency
/// explicitly excluded by the request or absent from source candidates remains
/// an error, with the same diagnostic used by REL and PHY planners.
pub fn apply_semantic_closure(
    candidates: &mut [RewriteCandidate],
    expanded: &BTreeSet<MemoryId>,
    exclude_ids: &[String],
) -> Result<(), String> {
    let excluded = exclude_ids
        .iter()
        .map(|s| parse_id(s, "Memory ID"))
        .collect::<Result<BTreeSet<_>, _>>()?;
    for id in expanded {
        if excluded.contains(&id.0) {
            return Err(format!(
                "explicitly excluded Memory {} is required by retained semantic evidence",
                id_hex(&id.0)
            ));
        }
        let Some((_, retain, reason, _)) = candidates
            .iter_mut()
            .find(|(candidate, ..)| candidate == id)
        else {
            return Err(format!(
                "semantic evidence requires unavailable Memory {}",
                id_hex(&id.0)
            ));
        };
        if !*retain {
            *retain = true;
            *reason = "required by retained semantic evidence closure".into();
        }
    }
    Ok(())
}

fn parse_id(value: &str, label: &str) -> Result<[u8; 32], String> {
    if value.len() != 64 {
        return Err(format!("{label} must contain 64 hexadecimal characters"));
    }
    let mut bytes = [0; 32];
    for (index, pair) in value.as_bytes().chunks_exact(2).enumerate() {
        let text = std::str::from_utf8(pair).map_err(|_| format!("invalid {label}"))?;
        bytes[index] = u8::from_str_radix(text, 16).map_err(|_| format!("invalid {label}"))?;
    }
    Ok(bytes)
}

fn id_hex(id: &[u8; 32]) -> String {
    id.iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(value: u8) -> MemoryId {
        MemoryId([value; 32])
    }

    fn text_id(value: u8) -> String {
        id_hex(&[value; 32])
    }

    #[test]
    fn explicit_selection_preserves_missing_id_error_order_and_exclusion_precedence() {
        let mut candidates = vec![(id(1), false, "base excluded".into(), vec![])];
        let error =
            apply_explicit_selection(&mut candidates, &[text_id(3)], &[text_id(2)]).unwrap_err();
        assert_eq!(
            error,
            format!(
                "explicitly included Memory {} does not exist in the source",
                text_id(3)
            )
        );

        let malformed =
            apply_explicit_selection(&mut candidates, &["bad".into()], &[]).unwrap_err();
        assert_eq!(
            malformed,
            "Memory ID must contain 64 hexadecimal characters"
        );

        apply_explicit_selection(&mut candidates, &[text_id(1)], &[text_id(1)]).unwrap();
        assert_eq!(candidates[0].1, false);
        assert_eq!(candidates[0].2, "explicitly excluded");
    }

    #[test]
    fn semantic_closure_rejects_excluded_or_missing_and_marks_new_requirements() {
        let mut candidates = vec![(id(1), false, "base excluded".into(), vec![])];
        let excluded =
            apply_semantic_closure(&mut candidates, &BTreeSet::from([id(1)]), &[text_id(1)])
                .unwrap_err();
        assert_eq!(
            excluded,
            format!(
                "explicitly excluded Memory {} is required by retained semantic evidence",
                text_id(1)
            )
        );
        let missing =
            apply_semantic_closure(&mut candidates, &BTreeSet::from([id(2)]), &[]).unwrap_err();
        assert_eq!(
            missing,
            format!(
                "semantic evidence requires unavailable Memory {}",
                text_id(2)
            )
        );

        apply_semantic_closure(&mut candidates, &BTreeSet::from([id(1)]), &[]).unwrap();
        assert_eq!(candidates[0].1, true);
        assert_eq!(
            candidates[0].2,
            "required by retained semantic evidence closure"
        );
    }
}
