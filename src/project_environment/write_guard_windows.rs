use super::*;
use std::fs::{self, OpenOptions};
use std::io::{Read, Seek, SeekFrom};
use std::os::windows::fs::{MetadataExt, OpenOptionsExt};
use std::path::{Component, Path, PathBuf};

const FILE_SHARE_READ: u32 = 0x0000_0001;
const FILE_FLAG_BACKUP_SEMANTICS: u32 = 0x0200_0000;
const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0400;

pub(crate) struct WriteGuard {
    target: PathBuf,
    _directories: Vec<File>,
    existing_file: Option<File>,
    is_directory: bool,
}

impl WriteGuard {
    pub(crate) fn for_path(scope: &ValidatedProjectScope, target: &Path) -> Result<Self, String> {
        Self::build(scope, target, true, false, false, false)
    }

    pub(crate) fn for_directory(
        scope: &ValidatedProjectScope,
        target: &Path,
    ) -> Result<Self, String> {
        Self::build(scope, target, false, true, false, false)
    }

    pub(crate) fn for_existing_file(
        scope: &ValidatedProjectScope,
        target: &Path,
    ) -> Result<Self, String> {
        Self::build(scope, target, false, false, false, false)
    }

    pub(crate) fn for_lore_ignore(scope: &ValidatedProjectScope) -> Result<Self, String> {
        if scope.repository_kind() != crate::ProjectRepositoryKind::Lore {
            return Err("Lore ignore guard requires a Lore project".into());
        }
        let target = scope.repository_root().join(".loreignore");
        Self::build(scope, &target, false, false, true, true)
    }

    fn build(
        scope: &ValidatedProjectScope,
        target: &Path,
        create_parents: bool,
        directory: bool,
        writable_file: bool,
        lore_ignore: bool,
    ) -> Result<Self, String> {
        scope.revalidate()?;
        let project = scope.project_dir();
        let root = scope.repository_root();
        let root = fs::canonicalize(root)
            .map_err(|error| format!("failed to resolve repository root: {error}"))?;
        let project = fs::canonicalize(project)
            .map_err(|error| format!("failed to resolve project folder: {error}"))?;
        if !project.starts_with(&root) {
            return Err("selected project folder is outside the repository root".into());
        }
        let target = if target.is_absolute() {
            target.to_path_buf()
        } else {
            return Err("project write target must be an absolute path".into());
        };
        if lore_ignore && target != root.join(".loreignore") {
            return Err("Lore ignore guard is restricted to the repository root".into());
        }
        let base = if lore_ignore { &root } else { &project };
        let relative = target
            .strip_prefix(base)
            .map_err(|_| "project write target is outside the selected subtree".to_string())?;
        let logical = target
            .strip_prefix(&root)
            .map_err(|_| "project write target is outside the repository root".to_string())?
            .to_string_lossy()
            .replace('\\', "/");
        if lore_ignore {
            // This exact repository-owned metadata path is the only scope exception.
        } else if directory {
            let canonical = fs::canonicalize(&target)
                .map_err(|error| format!("failed to resolve project directory: {error}"))?;
            if !canonical.starts_with(&project) {
                return Err("project directory is outside the selected subtree".into());
            }
        } else {
            scope.resolve_current_path(&logical)?;
        }
        let components = relative
            .components()
            .map(|component| match component {
                Component::Normal(name) => Ok(name.to_os_string()),
                _ => Err("project write target contains an invalid path component".into()),
            })
            .collect::<Result<Vec<_>, String>>()?;
        if !directory && components.is_empty() {
            return Err("project write target must name a file".into());
        }

        let mut directories = Vec::new();
        pin_directory_chain(&root, &mut directories)?;
        if !lore_ignore {
            pin_directory_chain(&project, &mut directories)?;
        }

        let mut parent = base.clone();
        let parent_components = if directory {
            components.len()
        } else {
            components.len().saturating_sub(1)
        };
        for component in components.iter().take(parent_components) {
            parent.push(component);
            if create_parents {
                match fs::create_dir(&parent) {
                    Ok(()) => {}
                    Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                        return Err(format!("project write parent disappeared: {error}"));
                    }
                    Err(error) => {
                        return Err(format!("failed to create project directory: {error}"));
                    }
                }
            }
            directories.push(open_directory(&parent)?);
        }

        // The read-only directory handles now prevent ancestor rename/reparse swaps.
        // Revalidate backend identity and containment while those handles are held.
        scope.revalidate()?;
        if lore_ignore {
            // The root and its ancestors remain pinned while backend identity is rechecked.
        } else if directory {
            if fs::canonicalize(&target).is_err() {
                return Err("project directory disappeared during validation".into());
            }
        } else {
            scope.resolve_current_path(&logical)?;
        }

        let (target, existing_file) = if directory {
            (target, None)
        } else {
            let filename = components.last().expect("file component checked");
            let mut target = parent;
            target.push(filename);
            let existing_file = match open_regular_file(&target, writable_file) {
                Ok(file) => Some(file),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
                Err(error) => {
                    return Err(format!("failed to pin project write target: {error}"));
                }
            };
            (target, existing_file)
        };
        Ok(Self {
            target,
            _directories: directories,
            existing_file,
            is_directory: directory,
        })
    }

    pub(crate) fn path(&self) -> &Path {
        &self.target
    }

    pub(crate) fn read_existing(&mut self) -> Result<Option<Vec<u8>>, String> {
        let Some(file) = &mut self.existing_file else {
            return Ok(None);
        };
        file.seek(SeekFrom::Start(0))
            .map_err(|error| format!("failed to seek existing project file: {error}"))?;
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes)
            .map_err(|error| format!("failed to read existing project file: {error}"))?;
        Ok(Some(bytes))
    }

    pub(crate) fn create_new(&self) -> Result<File, String> {
        if self.is_directory {
            return Err("cannot create a file through a directory guard".into());
        }
        let mut options = OpenOptions::new();
        options
            .read(true)
            .write(true)
            .create_new(true)
            .share_mode(FILE_SHARE_READ)
            .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT);
        options
            .open(&self.target)
            .map_err(|error| format!("failed to create project file safely: {error}"))
    }

    pub(crate) fn write_replace(&mut self, bytes: &[u8]) -> Result<(), String> {
        use std::io::{Seek, SeekFrom, Write};
        let file = match &mut self.existing_file {
            Some(file) => file,
            None => {
                self.existing_file = Some(self.create_new()?);
                self.existing_file.as_mut().expect("file was just created")
            }
        };
        file.set_len(0)
            .map_err(|error| format!("failed to truncate guarded file: {error}"))?;
        file.seek(SeekFrom::Start(0))
            .map_err(|error| format!("failed to seek guarded file: {error}"))?;
        file.write_all(bytes)
            .map_err(|error| format!("failed to write guarded file: {error}"))?;
        file.sync_all()
            .map_err(|error| format!("failed to sync guarded file: {error}"))
    }

    pub(crate) fn materialize(&mut self, bytes: &[u8]) -> Result<bool, String> {
        use std::io::Write;
        if let Some(existing) = self.read_existing()? {
            if existing == bytes {
                return Ok(false);
            }
            return Err("upload destination changed after preparation".into());
        }
        let mut file = self.create_new()?;
        file.write_all(bytes)
            .map_err(|error| format!("failed to write guarded project file: {error}"))?;
        file.sync_all()
            .map_err(|error| format!("failed to sync guarded project file: {error}"))?;
        self.existing_file = Some(file);
        Ok(true)
    }
}

fn open_directory(path: &Path) -> Result<File, String> {
    let mut options = OpenOptions::new();
    options
        .read(true)
        .share_mode(FILE_SHARE_READ)
        .custom_flags(FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT);
    let file = options
        .open(path)
        .map_err(|error| format!("failed to pin project directory: {error}"))?;
    let metadata = file
        .metadata()
        .map_err(|error| format!("failed to inspect pinned project directory: {error}"))?;
    if !metadata.is_dir() || metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
        return Err("project write path contains a reparse point or non-directory".into());
    }
    Ok(file)
}

fn pin_directory_chain(path: &Path, handles: &mut Vec<File>) -> Result<(), String> {
    let mut chain = path.ancestors().collect::<Vec<_>>();
    chain.reverse();
    for ancestor in chain {
        handles.push(open_directory(ancestor)?);
    }
    Ok(())
}

fn open_regular_file(path: &Path, writable: bool) -> std::io::Result<File> {
    let mut options = OpenOptions::new();
    options
        .read(true)
        .write(writable)
        .share_mode(FILE_SHARE_READ)
        .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT);
    let file = options.open(path)?;
    let metadata = file.metadata()?;
    if !metadata.is_file() || metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
        return Err(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            "project write target is a directory or reparse point",
        ));
    }
    Ok(file)
}
