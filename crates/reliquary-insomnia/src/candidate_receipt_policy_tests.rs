use crate::validate_semantic_authority;

#[test]
fn execution_receipts_require_progress_free_resulting_state() {
    assert_rejected(
        "Multiplayer prompt progress",
        "completed through 41",
        "The multiplayer lifecycle prompt sequence is completed through Prompt 41.",
    );
    assert_rejected(
        "Project status through Prompt 72",
        "Up to prompt 72 completed and everything seems to be working OK so far.",
        "Through Prompt 72, the project is generally working with minor visual bugs.",
    );
    assert_rejected(
        "Completed prompts 25 and 26",
        "25 and 26 are done. What about the rest?",
        "Prompts 25 and 26 are complete.",
    );

    assert_accepted(
        "Room type ownership",
        "COMPLETED PROMPT 31: networking now references rooms.Room directly",
        "Networking now references rooms.Room directly.",
    );
    assert_accepted(
        "Current project status",
        "Up to prompt 72 completed and everything seems to be working OK so far.",
        "The project is generally working, with minor visual bugs remaining.",
    );
    assert_accepted_category(
        "preference",
        "Calibrate prompt sizing",
        "Prompt 16 was the worst, over 6 minutes.",
        "Prompt 16 was notably too large; oversized prompts should be split.",
    );
}

fn assert_rejected(title: &str, source: &str, content: &str) {
    assert!(
        validate_semantic_authority("direct", "fact", title, source, content, false, false)
            .is_some()
    );
}

fn assert_accepted(title: &str, source: &str, content: &str) {
    assert!(
        validate_semantic_authority("direct", "fact", title, source, content, false, false)
            .is_none()
    );
}

fn assert_accepted_category(category: &str, title: &str, source: &str, content: &str) {
    assert!(
        validate_semantic_authority("direct", category, title, source, content, false, false)
            .is_none()
    );
}
