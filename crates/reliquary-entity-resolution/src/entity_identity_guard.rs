use std::collections::BTreeSet;

const GENERIC_SUFFIXES: &[&str] = &["package", "module", "component"];

fn identity_guard_keys(surface: &str, qualifier_tokens: &BTreeSet<String>) -> BTreeSet<String> {
    let mut keys = BTreeSet::new();
    append_surface_keys(surface, qualifier_tokens, &mut keys);

    if let Some(last) = surface
        .split(['/', '\\'])
        .filter(|part| !part.trim().is_empty())
        .next_back()
    {
        append_surface_keys(last, qualifier_tokens, &mut keys);
    }
    keys
}

fn single_token(value: &str) -> Option<String> {
    let tokens = tokens(value);
    (tokens.len() == 1).then(|| tokens[0].clone())
}

fn append_surface_keys(
    surface: &str,
    qualifier_tokens: &BTreeSet<String>,
    keys: &mut BTreeSet<String>,
) {
    let tokens = tokens(surface);
    if tokens.is_empty() {
        return;
    }
    keys.insert(tokens.concat());

    if tokens.len() > 1
        && tokens
            .last()
            .is_some_and(|value| GENERIC_SUFFIXES.contains(&value.as_str()))
    {
        keys.insert(tokens[..tokens.len() - 1].concat());
    }

    if tokens.len() > 1 && qualifier_tokens.contains(&tokens[0]) {
        keys.insert(tokens[1..].concat());
    }
}

fn tokens(value: &str) -> Vec<String> {
    let chars: Vec<_> = value.chars().collect();
    let mut out = Vec::new();
    let mut current = String::new();

    for (index, ch) in chars.iter().copied().enumerate() {
        if !ch.is_alphanumeric() {
            push_token(&mut out, &mut current);
            continue;
        }

        let split_camel = ch.is_uppercase()
            && !current.is_empty()
            && chars
                .get(index.wrapping_sub(1))
                .is_some_and(|prior| prior.is_lowercase())
            && chars.get(index + 1).is_some_and(|next| next.is_lowercase());
        if split_camel {
            push_token(&mut out, &mut current);
        }
        current.extend(ch.to_lowercase());
    }
    push_token(&mut out, &mut current);
    out
}

fn push_token(out: &mut Vec<String>, current: &mut String) {
    if !current.is_empty() {
        out.push(std::mem::take(current));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn guard_keys_strip_paths_generic_suffixes_and_language_qualifiers() {
        let qualifiers = BTreeSet::from(["go".to_owned()]);
        assert!(identity_guard_keys("services/game-server", &qualifiers).contains("gameserver"));
        assert!(identity_guard_keys("entities package", &qualifiers).contains("entities"));
        assert!(identity_guard_keys("Go Game Server", &qualifiers).contains("gameserver"));
        assert!(!identity_guard_keys("Server devtools", &qualifiers).contains("devtools"));
    }
}

pub fn creation_conflicts(
    current: &[reliquary_perception_store::Entity],
    surface: &str,
    kind: &str,
    limit: usize,
) -> Vec<reliquary_perception_store::Entity> {
    if kind == "principal" {
        return Vec::new();
    }
    let mut qualifiers = BTreeSet::new();
    for entity in current {
        if entity.kind == "programming_language" {
            for value in std::iter::once(entity.canonical_name.as_str())
                .chain(entity.aliases.iter().map(String::as_str))
            {
                if let Some(token) = single_token(value) {
                    qualifiers.insert(token);
                }
            }
        }
    }
    let wanted = identity_guard_keys(surface, &qualifiers);
    if wanted.is_empty() {
        return Vec::new();
    }

    let mut matches = Vec::new();
    for entity in current {
        if entity.kind != kind {
            continue;
        }
        let conflicts = std::iter::once(entity.canonical_name.as_str())
            .chain(entity.aliases.iter().map(String::as_str))
            .any(|candidate| {
                identity_guard_keys(candidate, &qualifiers)
                    .iter()
                    .any(|key| wanted.contains(key))
            });
        if conflicts {
            matches.push(entity.clone());
        }
    }
    matches.sort_by_key(|entity| entity.id);
    matches.truncate(limit);
    matches
}
