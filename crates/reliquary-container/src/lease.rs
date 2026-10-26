use super::ContainerError;
use std::fs::{self, File};
use std::path::Path;
use std::sync::Arc;

/// OS lock attached to the exact container file handle. Closing the handle on
/// normal drop or process exit releases the lock in the operating system.
#[derive(Debug)]
pub(super) struct Lease {
    exclusive: bool,
    identity: FileIdentity,
    token: [u8; 16],
    // Keep a duplicate of the handle on which the OS lock was acquired.
    // PhysicalReadSource clones this Arc so dropping the original Container
    // cannot release its lease while a read source is still alive.
    _locked_file: Arc<File>,
}

impl Lease {
    pub(super) fn exclusive(file: &File) -> Result<Self, ContainerError> {
        let identity = file_identity(file)?;
        if identity.as_pair().is_none() {
            return Err(ContainerError::LockingUnsupported);
        }
        lock_file(file, true)?;
        Ok(Self {
            exclusive: true,
            identity,
            token: *uuid::Uuid::new_v4().as_bytes(),
            _locked_file: Arc::new(file.try_clone()?),
        })
    }

    pub(super) fn shared(file: &File) -> Result<Self, ContainerError> {
        let identity = file_identity(file)?;
        if identity.as_pair().is_none() {
            return Err(ContainerError::LockingUnsupported);
        }
        lock_file(file, false)?;
        Ok(Self {
            exclusive: false,
            identity,
            token: *uuid::Uuid::new_v4().as_bytes(),
            _locked_file: Arc::new(file.try_clone()?),
        })
    }

    pub(super) fn is_exclusive(&self) -> bool {
        self.exclusive
    }
    pub(super) fn token(&self) -> [u8; 16] {
        self.token
    }
    pub(super) fn read_view(&self) -> Self {
        Self {
            exclusive: false,
            identity: self.identity,
            token: self.token,
            _locked_file: Arc::clone(&self._locked_file),
        }
    }
    pub(super) fn identity_pair(&self) -> Option<(u64, u64)> {
        self.identity.as_pair()
    }

    pub(super) fn ensure_path_identity(&self, path: &Path) -> Result<(), ContainerError> {
        let file = fs::OpenOptions::new()
            .read(true)
            .open(path)
            .map_err(|_| ContainerError::StaleIdentity)?;
        if self.identity == file_identity(&file)? {
            Ok(())
        } else {
            Err(ContainerError::StaleIdentity)
        }
    }
}

#[cfg(unix)]
fn lock_file(file: &File, exclusive: bool) -> Result<(), ContainerError> {
    if exclusive {
        file.try_lock().map_err(map_lock_error)
    } else {
        file.try_lock_shared().map_err(map_lock_error)
    }
}

#[cfg(windows)]
fn lock_file(file: &File, exclusive: bool) -> Result<(), ContainerError> {
    use std::os::windows::io::AsRawHandle;

    #[repr(C)]
    struct Overlapped {
        internal: usize,
        internal_high: usize,
        offset: u32,
        offset_high: u32,
        event: *mut std::ffi::c_void,
    }

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn LockFileEx(
            file: *mut std::ffi::c_void,
            flags: u32,
            reserved: u32,
            length_low: u32,
            length_high: u32,
            overlapped: *mut Overlapped,
        ) -> i32;
    }

    const LOCKFILE_FAIL_IMMEDIATELY: u32 = 0x0000_0001;
    const LOCKFILE_EXCLUSIVE_LOCK: u32 = 0x0000_0002;
    // Windows file lengths are bounded by signed 64-bit offsets. Lock one
    // byte at i64::MAX, immediately beyond the largest possible file byte.
    let offset = i64::MAX as u64;
    let mut overlapped = Overlapped {
        internal: 0,
        internal_high: 0,
        offset: offset as u32,
        offset_high: (offset >> 32) as u32,
        event: std::ptr::null_mut(),
    };
    let mut flags = LOCKFILE_FAIL_IMMEDIATELY;
    if exclusive {
        flags |= LOCKFILE_EXCLUSIVE_LOCK;
    }
    // SAFETY: `file` owns a live handle and `overlapped` remains valid for the
    // synchronous fail-immediately operation. The one-byte range is at EOF+
    // one even for the largest representable file, so it never locks content.
    let locked = unsafe { LockFileEx(file.as_raw_handle(), flags, 0, 1, 0, &mut overlapped) };
    if locked != 0 {
        return Ok(());
    }
    let error = std::io::Error::last_os_error();
    match error.raw_os_error() {
        Some(32 | 33) => Err(ContainerError::WriterConflict),
        _ if error.kind() == std::io::ErrorKind::Unsupported => {
            Err(ContainerError::LockingUnsupported)
        }
        _ => Err(ContainerError::Io(error)),
    }
}

#[cfg(not(any(unix, windows)))]
fn lock_file(_file: &File, _exclusive: bool) -> Result<(), ContainerError> {
    Err(ContainerError::LockingUnsupported)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum FileIdentity {
    #[cfg(unix)]
    Unix { device: u64, inode: u64 },
    #[cfg(windows)]
    Windows { volume: u64, index: u64 },
    #[cfg(not(any(unix, windows)))]
    Unsupported,
}

impl FileIdentity {
    pub(super) fn as_pair(self) -> Option<(u64, u64)> {
        match self {
            #[cfg(unix)]
            Self::Unix { device, inode } => Some((device, inode)),
            #[cfg(windows)]
            Self::Windows { volume, index } => Some((volume, index)),
            #[cfg(not(any(unix, windows)))]
            Self::Unsupported => None,
        }
    }
}

pub(super) fn file_identity(file: &File) -> Result<FileIdentity, ContainerError> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        let metadata = file.metadata()?;
        return Ok(FileIdentity::Unix {
            device: metadata.dev(),
            inode: metadata.ino(),
        });
    }
    #[cfg(windows)]
    {
        use std::os::windows::io::AsRawHandle;
        #[repr(C)]
        struct Information {
            attributes: u32,
            creation: [u32; 2],
            access: [u32; 2],
            write: [u32; 2],
            volume: u32,
            size_high: u32,
            size_low: u32,
            links: u32,
            index_high: u32,
            index_low: u32,
        }
        #[link(name = "kernel32")]
        unsafe extern "system" {
            fn GetFileInformationByHandle(
                handle: *mut std::ffi::c_void,
                information: *mut Information,
            ) -> i32;
        }
        let mut information = std::mem::MaybeUninit::<Information>::uninit();
        // SAFETY: `file` owns a live handle and the OS fills the declared structure.
        if unsafe { GetFileInformationByHandle(file.as_raw_handle(), information.as_mut_ptr()) }
            == 0
        {
            return Err(ContainerError::Io(std::io::Error::last_os_error()));
        }
        // SAFETY: success initializes every structure field.
        let information = unsafe { information.assume_init() };
        let index = (u64::from(information.index_high) << 32) | u64::from(information.index_low);
        return Ok(FileIdentity::Windows {
            volume: u64::from(information.volume),
            index,
        });
    }
    #[cfg(not(any(unix, windows)))]
    {
        Ok(FileIdentity::Unsupported)
    }
}

#[cfg(unix)]
fn map_lock_error(error: std::fs::TryLockError) -> ContainerError {
    match error {
        std::fs::TryLockError::WouldBlock => ContainerError::WriterConflict,
        std::fs::TryLockError::Error(error) if error.kind() == std::io::ErrorKind::Unsupported => {
            ContainerError::LockingUnsupported
        }
        std::fs::TryLockError::Error(error) => ContainerError::Io(error),
    }
}
