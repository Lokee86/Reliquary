use super::*;
use rustix::fs::{self as rfs, CWD, Mode, OFlags};
use std::fs;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Component, PathBuf};

pub(crate) struct WriteGuard {
    target: PathBuf,
    parent: File,
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
        Self::build(
            scope,
            &scope.repository_root().join(".loreignore"),
            false,
            false,
            true,
            true,
        )
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
        let root = fs::canonicalize(scope.repository_root()).map_err(|e| e.to_string())?;
        let project = fs::canonicalize(scope.project_dir()).map_err(|e| e.to_string())?;
        if !target.is_absolute() || !project.starts_with(&root) {
            return Err("project write target is outside the selected subtree".into());
        }
        if !directory && !lore_ignore {
            let logical = target
                .strip_prefix(&root)
                .map_err(|_| "project write target is outside the repository root".to_string())?
                .to_string_lossy()
                .replace('\\', "/");
            scope.resolve_current_path(&logical)?;
        }
        if lore_ignore && target != root.join(".loreignore") {
            return Err("invalid Lore ignore path".into());
        }
        let base = if lore_ignore { &root } else { &project };
        let relative = target
            .strip_prefix(base)
            .map_err(|_| "project write target is outside the selected subtree".to_string())?;
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
        let mut root_fd = open_absolute_directory(&root, &mut directories)?;
        let project_relative = base
            .strip_prefix(&root)
            .map_err(|_| "selected project folder is outside the repository root".to_string())?;
        for component in project_relative.components() {
            let Component::Normal(name) = component else {
                return Err("project path contains an invalid component".into());
            };
            root_fd = open_child_directory(&root_fd, name).map_err(|e| e.to_string())?;
            directories.push(root_fd.try_clone().map_err(|e| e.to_string())?);
        }
        let mut parent = root_fd;
        let parent_count = if directory {
            components.len()
        } else {
            components.len().saturating_sub(1)
        };
        for component in components.iter().take(parent_count) {
            match open_child_directory(&parent, component) {
                Ok(child) => parent = child,
                Err(error) if error == rustix::io::Errno::NOENT && create_parents => {
                    rfs::mkdirat(&parent, component, Mode::from_bits_retain(0o700))
                        .map_err(|e| format!("failed to create project directory: {e}"))?;
                    parent = open_child_directory(&parent, component).map_err(|e| e.to_string())?;
                }
                Err(error) => return Err(error.to_string()),
            }
            directories.push(parent.try_clone().map_err(|e| e.to_string())?);
        }
        if directory && components.is_empty() {
            directories.push(parent.try_clone().map_err(|e| e.to_string())?);
        }
        scope.revalidate()?;
        let final_target = if directory {
            target.to_path_buf()
        } else {
            let mut value = target.parent().unwrap_or(&project).to_path_buf();
            value.push(components.last().expect("file component checked"));
            value
        };
        let existing_file = if directory {
            None
        } else {
            match open_child_file(
                &parent,
                components.last().expect("file component checked"),
                writable_file,
            ) {
                Ok(file) => Some(file),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
                Err(error) => {
                    return Err(format!("failed to pin project write target: {error}"));
                }
            }
        };
        Ok(Self {
            target: final_target,
            parent,
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
        file.seek(SeekFrom::Start(0)).map_err(|e| e.to_string())?;
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes).map_err(|e| e.to_string())?;
        Ok(Some(bytes))
    }

    pub(crate) fn create_new(&self) -> Result<File, String> {
        if self.is_directory {
            return Err("cannot create a file through a directory guard".into());
        }
        let name = self
            .target
            .file_name()
            .ok_or_else(|| "project write target has no filename".to_string())?;
        let fd = rfs::openat(
            &self.parent,
            name,
            OFlags::RDWR | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::from_bits_retain(0o666),
        )
        .map_err(|e| format!("failed to create project file safely: {e}"))?;
        Ok(File::from(fd))
    }

    pub(crate) fn write_replace(&mut self, bytes: &[u8]) -> Result<(), String> {
        let file = match &mut self.existing_file {
            Some(file) => file,
            None => {
                self.existing_file = Some(self.create_new()?);
                self.existing_file.as_mut().unwrap()
            }
        };
        file.set_len(0).map_err(|e| e.to_string())?;
        file.seek(SeekFrom::Start(0)).map_err(|e| e.to_string())?;
        file.write_all(bytes).map_err(|e| e.to_string())?;
        file.sync_all().map_err(|e| e.to_string())
    }

    pub(crate) fn materialize(&mut self, bytes: &[u8]) -> Result<bool, String> {
        if let Some(existing) = self.read_existing()? {
            return if existing == bytes {
                Ok(false)
            } else {
                Err("upload destination changed after preparation".into())
            };
        }
        let mut file = self.create_new()?;
        file.write_all(bytes).map_err(|e| e.to_string())?;
        file.sync_all().map_err(|e| e.to_string())?;
        self.existing_file = Some(file);
        Ok(true)
    }
}

fn open_absolute_directory(path: &Path, handles: &mut Vec<File>) -> Result<File, String> {
    let mut current = File::from(
        rfs::openat(
            CWD,
            "/",
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .map_err(|e| e.to_string())?,
    );
    handles.push(current.try_clone().map_err(|e| e.to_string())?);
    for component in path.components() {
        match component {
            Component::RootDir => {}
            Component::Normal(name) => {
                current = open_child_directory(&current, name).map_err(|e| e.to_string())?;
                handles.push(current.try_clone().map_err(|e| e.to_string())?);
            }
            _ => return Err("repository root contains an invalid path component".into()),
        }
    }
    Ok(current)
}

fn open_child_directory(parent: &File, name: &std::ffi::OsStr) -> Result<File, rustix::io::Errno> {
    let fd = rfs::openat(
        parent,
        name,
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    )?;
    Ok(File::from(fd))
}

fn open_child_file(parent: &File, name: &std::ffi::OsStr, writable: bool) -> std::io::Result<File> {
    let access = if writable {
        OFlags::RDWR
    } else {
        OFlags::RDONLY
    };
    let fd = rfs::openat(
        parent,
        name,
        access | OFlags::NOFOLLOW | OFlags::CLOEXEC | OFlags::NONBLOCK,
        Mode::empty(),
    )
    .map_err(std::io::Error::from)?;
    let file = File::from(fd);
    if !file.metadata()?.is_file() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            "project write target is not a regular file",
        ));
    }
    Ok(file)
}
