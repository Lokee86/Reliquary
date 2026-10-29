#[path = "project_environment/lifecycle.rs"]
mod lifecycle;

pub(crate) use lifecycle::{
    adopt_project_rel, create_project_rel, inspect_project_adoption_source,
};
pub use reliquary_project_environment::ProjectRepositoryMode;
pub(crate) use reliquary_project_environment::{
    checkpoint_manual_lore, current_revision_for, prepare_project_repository, prepare_upload,
    read_project_file, repository_root,
};
use std::path::PathBuf;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectAdoptionSource {
    pub path: PathBuf,
    pub owner_id: String,
}
