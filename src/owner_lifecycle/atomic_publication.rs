use std::fs::{self, File};
use std::io;
use std::path::Path;

pub(crate) fn atomic_replace(from: &Path, to: &Path) -> io::Result<()> {
    fs::rename(from, to)
}

#[cfg(unix)]
pub(crate) fn sync_parent(parent: &Path) -> io::Result<()> {
    File::open(parent)?.sync_all()
}

#[cfg(not(unix))]
pub(crate) fn sync_parent(_parent: &Path) -> io::Result<()> {
    Ok(())
}
