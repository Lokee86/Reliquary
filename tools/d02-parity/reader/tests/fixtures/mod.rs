use reliquary_container::ReliquaryScope;
use reliquary_owner_session::RelWriter;
use std::path::Path;

pub fn create_project_rel(path: &Path, owner: [u8; 16]) -> RelWriter {
    RelWriter::create_path(path, ReliquaryScope::Project, owner).unwrap()
}
