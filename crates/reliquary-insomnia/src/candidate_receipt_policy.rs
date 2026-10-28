use super::candidate_text::normalize;

pub(crate) fn violates_execution_receipt_policy(title: &str, content: &str) -> bool {
    let combined = format!(" {} {} ", normalize(title), normalize(content));
    if has_numbered_progress(&combined) {
        return true;
    }

    let has_transient_unit = [
        " prompt ",
        " prompts ",
        " phase ",
        " phases ",
        " step ",
        " steps ",
        " test run ",
        " tests ",
        " commit ",
        " push ",
        " merge ",
        " verification ",
        " cleanup ",
        " worktree ",
        " branch ",
        " milestone ",
    ]
    .iter()
    .any(|marker| combined.contains(marker));
    let has_completion = [
        " completed ",
        " complete ",
        " done ",
        " finished ",
        " passed ",
        " pushed ",
        " merged ",
        " committed ",
        " verified ",
        " reached ",
        " clean ",
    ]
    .iter()
    .any(|marker| combined.contains(marker));
    has_transient_unit && has_completion && !has_durable_state_signal(content)
}

fn has_numbered_progress(combined: &str) -> bool {
    let tokens: Vec<_> = combined.split_whitespace().collect();
    let has_numbered_unit = tokens.iter().enumerate().any(|(index, token)| {
        matches!(
            *token,
            "prompt" | "prompts" | "phase" | "phases" | "step" | "steps"
        ) && tokens.iter().skip(index + 1).take(4).any(|candidate| {
            candidate
                .chars()
                .next()
                .is_some_and(|ch| ch.is_ascii_digit())
        })
    });
    if !has_numbered_unit {
        return false;
    }

    let padded = format!(" {combined} ");
    [
        " completed ",
        " complete ",
        " done ",
        " finished ",
        " passed ",
        " reached ",
        " checkpoint ",
        " milestone ",
        " progress ",
        " through ",
        " up to ",
    ]
    .iter()
    .any(|marker| padded.contains(marker))
}

fn has_durable_state_signal(content: &str) -> bool {
    let padded = format!(" {} ", normalize(content));
    [
        " uses ",
        " owns ",
        " references ",
        " stores ",
        " requires ",
        " configured ",
        " now uses ",
        " moved to ",
        " lives in ",
        " returns ",
        " sends ",
        " persists ",
        " wired through ",
        " routes through ",
        " supports ",
        " rejects ",
        " accepts ",
    ]
    .iter()
    .any(|marker| padded.contains(marker))
}
