pub(crate) fn informative_tokens(value: &str) -> usize {
    normalize(value)
        .split_whitespace()
        .filter(|word| {
            !matches!(
                *word,
                "a" | "an"
                    | "and"
                    | "the"
                    | "to"
                    | "of"
                    | "for"
                    | "in"
                    | "on"
                    | "with"
                    | "i"
                    | "we"
                    | "you"
                    | "that"
                    | "this"
                    | "it"
                    | "them"
                    | "those"
                    | "these"
                    | "do"
                    | "then"
                    | "alright"
                    | "ok"
                    | "okay"
                    | "please"
                    | "just"
                    | "good"
                    | "sounds"
            )
        })
        .count()
}

pub(crate) fn normalize(value: &str) -> String {
    value
        .to_lowercase()
        .chars()
        .map(|ch| {
            if ch.is_alphanumeric() || ch == '\'' {
                ch
            } else {
                ' '
            }
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}
