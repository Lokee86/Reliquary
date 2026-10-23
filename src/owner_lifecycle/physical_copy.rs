//! Exact physical copy with source leases and identity-owned failure cleanup.
use crate::{Container, ContainerError};
use std::{fs, path::Path};
pub(crate) fn copy(source: &Path, output: &Path) -> Result<Container, ContainerError> {
    let source_guard = Container::open_read_only(source)?;
    source_guard.validate_path_identity()?;
    let mut destination = source_guard.create_empty_like(output)?;
    let owned = destination.read_source()?;
    let result = (|| {
        let mut cursor = source_guard.physical_cursor()?;
        while let Some(frame) = cursor.next_frame()? {
            destination.append(&frame.payload)?;
        }
        destination.sync()?;
        source_guard.validate_path_identity()?;
        destination.validate_path_identity()?;
        Ok(destination)
    })();
    if result.is_err() && owned.validate_path_identity().is_ok() {
        let _ = fs::remove_file(output);
    }
    result
}
