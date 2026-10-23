//! Race-resistant guard for project write paths.
use super::ValidatedProjectScope;
use std::fs::File;
use std::path::Path;

#[cfg(unix)]
#[path = "write_guard_unix.rs"]
mod unix;
#[cfg(windows)]
#[path = "write_guard_windows.rs"]
mod windows;
#[cfg(not(any(windows, unix)))]
mod unsupported {
    use super::*;
    pub(crate) struct WriteGuard;
    impl WriteGuard {
        pub(crate) fn for_path(_: &ValidatedProjectScope, _: &Path) -> Result<Self, String> {
            Err("race-resistant project writes are unsupported on this platform".into())
        }
        pub(crate) fn for_directory(_: &ValidatedProjectScope, _: &Path) -> Result<Self, String> {
            Err("race-resistant project writes are unsupported on this platform".into())
        }
        pub(crate) fn for_existing_file(
            _: &ValidatedProjectScope,
            _: &Path,
        ) -> Result<Self, String> {
            Err("race-resistant project writes are unsupported on this platform".into())
        }
        pub(crate) fn for_lore_ignore(_: &ValidatedProjectScope) -> Result<Self, String> {
            Err("race-resistant project writes are unsupported on this platform".into())
        }
        pub(crate) fn path(&self) -> &Path {
            unreachable!()
        }
        pub(crate) fn read_existing(&mut self) -> Result<Option<Vec<u8>>, String> {
            Err("unsupported platform".into())
        }
        pub(crate) fn create_new(&self) -> Result<File, String> {
            Err("unsupported platform".into())
        }
        pub(crate) fn write_replace(&mut self, _: &[u8]) -> Result<(), String> {
            Err("unsupported platform".into())
        }
        pub(crate) fn materialize(&mut self, _: &[u8]) -> Result<bool, String> {
            Err("unsupported platform".into())
        }
    }
}
#[cfg(unix)]
pub(crate) use unix::WriteGuard;
#[cfg(not(any(windows, unix)))]
pub(crate) use unsupported::WriteGuard;
#[cfg(windows)]
pub(crate) use windows::WriteGuard;
