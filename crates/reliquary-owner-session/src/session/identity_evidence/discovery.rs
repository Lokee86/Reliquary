use super::*;

impl RelSession {
    /// A seekable page of candidate keys for active, non-principal local Entities.
    /// These keys only nominate comparisons. Keep them in the authorized discovery
    /// context and invalidate pages whenever the evidence cut changes.
    pub fn identity_discovery_keys(
        &self,
        after: Option<EntityId>,
        limit: usize,
    ) -> Result<Vec<(EntityId, Vec<[u8; 32]>)>, OwnerSessionError> {
        self.common
            .perception
            .entities_after(after)
            .filter(|entity| {
                entity.kind != PRINCIPAL_ENTITY_KIND && self.entity_revision_source_eligible(entity)
            })
            .take(limit)
            .map(|entity| {
                let mut keys = BTreeSet::new();
                for label in std::iter::once(&entity.canonical_name).chain(&entity.aliases) {
                    let label = normalized_entity_label(label);
                    if !label.is_empty() {
                        keys.insert(discovery_key(b"label", &label));
                    }
                }
                for mention in self
                    .common
                    .identity_mentions(&entity.into(), |id| !self.memory_source_removed(id))?
                {
                    keys.extend(
                        mention_identifiers(&mention)
                            .map(|value| discovery_key(b"identifier", &value.to_ascii_lowercase())),
                    );
                }
                Ok((entity.id, keys.into_iter().collect()))
            })
            .collect()
    }
}

fn normalized_entity_label(label: &str) -> String {
    label
        .split_whitespace()
        .map(str::to_lowercase)
        .collect::<Vec<_>>()
        .join(" ")
}

fn discovery_key(domain: &[u8], value: &str) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(b"reliquary-entity-identity-discovery-v1\0");
    hash.update(domain);
    hash.update([0]);
    hash.update(value.as_bytes());
    hash.finalize().into()
}

fn identifier_character(ch: char) -> bool {
    ch.is_alphanumeric() || matches!(ch, '-' | '_' | '/' | '.')
}

fn mention_identifiers(mention: &IdentityMentionEvidence) -> impl Iterator<Item = &str> {
    // An excerpt can round each window edge inward by up to three UTF-8 bytes.
    const MIN_FULL_WINDOW_BYTES: usize = MAX_IDENTITY_MENTION_QUOTE_BYTES - 2 * 3;
    let span = mention.memory.reference.span;
    let excerpt = &mention.quote;
    excerpt
        .split_inclusive(|ch| !identifier_character(ch))
        .scan(0_usize, move |start, segment| {
            let offset = *start;
            *start += segment.len();
            let token = segment.trim_end_matches(|ch| !identifier_character(ch));
            Some((offset, token))
        })
        .filter_map(move |(offset, token)| {
            let span = span?;
            let start = span.start_byte as usize + offset;
            let end = start + token.len();
            let full_window = excerpt.len() >= MIN_FULL_WINDOW_BYTES;
            if (full_window && offset == 0 && span.start_byte != 0)
                || (full_window && offset + token.len() == excerpt.len())
                || (start < mention.key.end_byte as usize && end > mention.key.start_byte as usize)
            {
                return None;
            }
            let token = token.trim_end_matches('.');
            distinctive_identifier(token).then_some(token)
        })
}

fn distinctive_identifier(token: &str) -> bool {
    if !(5..=96).contains(&token.len()) {
        return false;
    }
    if token.contains('.') {
        return package_identifier(token);
    }
    let mut parts = token.split(['-', '_', '/']);
    let head = parts.next().unwrap_or_default();
    let prefix = head.trim_end_matches(|ch: char| ch.is_ascii_digit());
    if !head.as_bytes().first().is_some_and(u8::is_ascii_alphabetic)
        || !head.bytes().all(|byte| byte.is_ascii_alphanumeric())
        || head.len() < 2
        || ["v", "ver", "version"]
            .iter()
            .any(|version_prefix| prefix.eq_ignore_ascii_case(version_prefix))
    {
        return false;
    }
    let mut numeric_serial = false;
    for part in parts {
        if part.is_empty() || !part.bytes().all(|byte| byte.is_ascii_alphanumeric()) {
            return false;
        }
        // A plain word and year such as "contract-2026" is ordinary prose.
        numeric_serial |= part.len() >= 3
            && part.bytes().all(|byte| byte.is_ascii_digit())
            && !(head == prefix
                && part.len() == 4
                && part
                    .parse::<u16>()
                    .is_ok_and(|year| (1900..=2099).contains(&year)));
    }
    numeric_serial
}

fn package_identifier(token: &str) -> bool {
    let mut parts = token.split('.');
    let root = parts.next().unwrap_or_default();
    // Reverse-domain packages have a recognizable root and at least two names.
    ["com", "org", "net", "edu", "gov", "io", "dev", "app"]
        .iter()
        .any(|domain| root.eq_ignore_ascii_case(domain))
        && parts.clone().count() >= 2
        && parts.all(|part| {
            part.as_bytes().first().is_some_and(u8::is_ascii_alphabetic)
                && part
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
        })
}

#[cfg(test)]
#[test]
fn discovery_uses_exact_label_tokens_and_structured_serials() {
    assert_eq!(
        normalized_entity_label("  ÉLODIE\t  Lantern "),
        "élodie lantern"
    );
    assert_ne!(normalized_entity_label("Élodie-Lantern"), "élodie lantern");
    assert_ne!(
        normalized_entity_label("C++"),
        normalized_entity_label("C#")
    );
    assert_ne!(normalized_entity_label("C++"), normalized_entity_label("C"));
    assert_ne!(
        discovery_key(b"label", "acme"),
        discovery_key(b"identifier", "acme")
    );

    for identifier in [
        "M42-7816",
        "m42-7816",
        "CASE_04762",
        "ACME/AT-09716",
        "ABC-123",
        "abc-123",
        "com.arden.editor",
        "ORG.ARDEN.EDITOR",
    ] {
        assert!(distinctive_identifier(identifier), "{identifier}");
    }
    for ordinary in [
        "Lantern",
        "CONTRACT",
        "contract-2026",
        "2026-11-24",
        "2026/11/24",
        "v2.1.0",
        "v2.1.0-rc.7816",
        "www.example.com",
        "com.editor",
        "version-2026",
        "V2-2026",
        "M42",
        "M42--7816",
        "ÉM42-7816",
    ] {
        assert!(!distinctive_identifier(ordinary), "{ordinary}");
    }
    assert_ne!(
        discovery_key(b"identifier", "m42-7816"),
        discovery_key(b"identifier", "m42_7816")
    );
}
