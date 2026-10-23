use std::fmt;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RewriteError(pub String);
impl fmt::Display for RewriteError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
impl std::error::Error for RewriteError {}
impl From<String> for RewriteError {
    fn from(value: String) -> Self {
        Self(value)
    }
}
