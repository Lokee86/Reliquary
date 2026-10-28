/// Credential authentication category used by configured model providers.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ModelAuthKind {
    ChatGptDeviceCode,
    ApiKey,
}
