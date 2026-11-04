//! Read-only access to repository-owned project history.
//!
//! Git remains authoritative for repository identity, revisions, trees, and file
//! bytes. This crate validates an existing association and returns bytes from an
//! exact Git tree; it does not create or persist project history.

mod git_history;
mod git_repository;
#[cfg(feature = "lore-history")]
mod lore_history;

pub use git_history::{read_git_revision_file, read_project_file_reference};
pub use git_repository::{current_git_revision, snapshot_git_project_file};
#[cfg(feature = "lore-history")]
pub use lore_history::{checkpoint_lore_project, read_lore_project_file};
