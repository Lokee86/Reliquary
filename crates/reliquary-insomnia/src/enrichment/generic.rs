const NON_REUSABLE_GENERIC_SURFACES: &[&str] = &[
    "agent",
    "the agent",
    "application",
    "the application",
    "assistant",
    "the assistant",
    "client",
    "the client",
    "code",
    "the code",
    "data",
    "the data",
    "documentation",
    "the documentation",
    "entry",
    "the entry",
    "file",
    "the file",
    "flow",
    "the flow",
    "implementation",
    "the implementation",
    "lane",
    "the lane",
    "pipeline",
    "the pipeline",
    "project",
    "the project",
    "repository",
    "the repository",
    "request",
    "the request",
    "response",
    "the response",
    "script",
    "the script",
    "scripts",
    "the scripts",
    "server",
    "the server",
    "session",
    "the session",
    "shader",
    "the shader",
    "state",
    "the state",
    "status",
    "the status",
    "system",
    "the system",
    "tooling",
    "the tooling",
    "login status",
    "the login status",
    "requested state",
    "the requested state",
    "relevant repository",
    "the relevant repository",
    "existing server",
    "the existing server",
];

const NON_REUSABLE_ABSTRACT_HEADS: &[&str] = &[
    "flow",
    "lane",
    "mechanism",
    "request",
    "response",
    "seam",
    "state",
    "status",
];

pub(super) fn is_non_reusable_generic_surface(text: &str) -> bool {
    let text = text.trim();
    let lower = text.to_ascii_lowercase();
    if text.split_whitespace().count() > 1
        && lower
            .split_whitespace()
            .next_back()
            .is_some_and(|head| NON_REUSABLE_ABSTRACT_HEADS.contains(&head))
    {
        return true;
    }
    if lower.starts_with("the ") {
        return NON_REUSABLE_GENERIC_SURFACES.contains(&lower.as_str());
    }
    text == lower && NON_REUSABLE_GENERIC_SURFACES.contains(&lower.as_str())
}
