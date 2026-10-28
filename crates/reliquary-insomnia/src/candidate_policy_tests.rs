use crate::validate_semantic_authority;

#[test]
fn authority_and_grounding_sources_have_distinct_roles() {
    assert!(
        validate_semantic_authority(
            "adoption",
            "decision",
            "Server state",
            "alright, that works",
            "The server returns the updated Player state.",
            false,
            true,
        )
        .is_some()
    );
    assert!(
        validate_semantic_authority(
            "direct",
            "decision",
            "Server state",
            "The server returns the updated Player state.",
            "The server returns the updated Player state.",
            true,
            false,
        )
        .is_some()
    );
    assert!(
        validate_semantic_authority(
            "correction",
            "decision",
            "ShipStats",
            "we do want to add it now",
            "Add ShipStats now.",
            false,
            true,
        )
        .is_none()
    );
}

#[test]
fn bare_retention_requires_authority_source_but_inline_retention_does_not() {
    assert!(
        validate_semantic_authority(
            "retention",
            "preference",
            "Preference",
            "Remember that.",
            "The user prefers concise answers.",
            false,
            true,
        )
        .is_some()
    );
    assert!(
        validate_semantic_authority(
            "retention",
            "preference",
            "Preference",
            "Remember that I prefer concise answers.",
            "The user prefers concise answers.",
            false,
            false,
        )
        .is_none()
    );
    assert!(
        validate_semantic_authority(
            "retention",
            "location",
            "Workspace root",
            "Remember this path: C:\\!bin.",
            "The workspace root is C:\\!bin.",
            false,
            false,
        )
        .is_none()
    );
}

#[test]
fn vague_turn_requires_authority_or_grounding_provenance() {
    assert!(
        validate_semantic_authority(
            "direct",
            "decision",
            "Call resolution",
            "alright, do that then",
            "Improve macro-mediated call resolution without sacrificing precision.",
            false,
            false,
        )
        .is_some()
    );
    assert!(
        validate_semantic_authority(
            "correction",
            "correction",
            "Deleted source assets",
            "you fucking deleted them",
            "The referenced source assets were deleted.",
            false,
            true,
        )
        .is_none()
    );
}

#[test]
fn unsupported_questions_are_rejected_but_asserted_tag_question_is_kept() {
    assert!(
        validate_semantic_authority(
            "direct",
            "decision",
            "Spawner ownership",
            "say what? add local spawners and then transfer logic to the server?",
            "Add local spawners and transfer spawn logic to the server.",
            false,
            false,
        )
        .is_some()
    );
    assert!(
        validate_semantic_authority(
            "direct",
            "constraint",
            "Server-authoritative collision",
            "they'll have to because collision is server-authoritative, right?",
            "Collision handling has to remain server-authoritative.",
            false,
            false,
        )
        .is_none()
    );
    assert!(
        validate_semantic_authority(
            "direct",
            "fact",
            "Language",
            "The project uses Rust. Is that okay?",
            "The project uses Rust.",
            false,
            false,
        )
        .is_none()
    );
    assert!(
        validate_semantic_authority(
            "direct",
            "correction",
            "TextureRect UI",
            "I'm using a TextureRect, right?",
            "The UI uses a TextureRect rather than Sprite2D.",
            false,
            false,
        )
        .is_none()
    );
    assert!(
        validate_semantic_authority(
            "direct",
            "constraint",
            "Custom room codes deferred",
            "it's also not accepting custom room codes at the moment, that's not major is it?",
            "Custom room codes are deferred.",
            false,
            false,
        )
        .is_some()
    );
}
