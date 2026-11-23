use std::fs::{self, File};
use std::io;
use std::path::{Path, PathBuf};

pub(super) fn root() -> io::Result<PathBuf> {
    let base = directories::BaseDirs::new().ok_or(io::ErrorKind::NotFound)?;
    let cache = base.cache_dir();
    fs::create_dir_all(cache)?;
    let root = cache.canonicalize()?.join("reliquary-owner-checkpoints");
    platform::ensure_directory(&root)?;
    Ok(root)
}

pub(super) fn open(path: &Path) -> io::Result<File> {
    platform::open_file(path)
}

#[cfg(test)]
pub(super) fn test_directory(path: &Path) -> io::Result<()> {
    platform::ensure_directory(path)
}

#[cfg(unix)]
mod platform {
    use std::fs::{self, DirBuilder, File, Metadata, OpenOptions};
    use std::io;
    use std::os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt};
    use std::path::Path;

    fn owned_private(metadata: &Metadata) -> bool {
        // SAFETY: geteuid reads the effective identity of the current process.
        metadata.uid() == unsafe { libc::geteuid() } && metadata.mode() & 0o077 == 0
    }

    pub(super) fn ensure_directory(path: &Path) -> io::Result<()> {
        match DirBuilder::new().mode(0o700).create(path) {
            Ok(()) => {}
            Err(err) if err.kind() == io::ErrorKind::AlreadyExists => {}
            Err(err) => return Err(err),
        }
        let metadata = fs::symlink_metadata(path)?;
        if metadata.is_dir() && owned_private(&metadata) {
            Ok(())
        } else {
            Err(io::ErrorKind::PermissionDenied.into())
        }
    }

    pub(super) fn open_file(path: &Path) -> io::Result<File> {
        let file = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
            .open(path)?;
        let metadata = file.metadata()?;
        if metadata.is_file() && metadata.nlink() == 1 && owned_private(&metadata) {
            Ok(file)
        } else {
            Err(io::ErrorKind::PermissionDenied.into())
        }
    }
}

#[cfg(windows)]
mod platform {
    use std::ffi::{OsStr, c_void};
    use std::fs::{File, OpenOptions};
    use std::io;
    use std::os::windows::ffi::OsStrExt;
    use std::os::windows::fs::{MetadataExt, OpenOptionsExt};
    use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
    use std::path::Path;
    use std::ptr::{addr_of, null_mut};
    use windows_sys::Win32::Foundation::{ERROR_SUCCESS, LocalFree};
    use windows_sys::Win32::Security::Authorization::{
        ConvertSidToStringSidW, ConvertStringSecurityDescriptorToSecurityDescriptorW,
        GetSecurityInfo, SE_FILE_OBJECT,
    };
    use windows_sys::Win32::Security::{
        ACCESS_ALLOWED_ACE, ACL, DACL_SECURITY_INFORMATION, GetAce, GetTokenInformation,
        IsWellKnownSid, OWNER_SECURITY_INFORMATION, PSID, SECURITY_ATTRIBUTES, TOKEN_QUERY,
        TOKEN_USER, TokenUser, WinLocalSystemSid,
    };
    use windows_sys::Win32::Storage::FileSystem::{
        CreateDirectoryW, FILE_ATTRIBUTE_REPARSE_POINT, FILE_FLAG_BACKUP_SEMANTICS,
        FILE_FLAG_OPEN_REPARSE_POINT, FILE_GENERIC_READ, READ_CONTROL,
    };
    use windows_sys::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

    struct LocalAllocation(*mut c_void);

    impl Drop for LocalAllocation {
        fn drop(&mut self) {
            // SAFETY: these buffers were returned by Win32 local allocation APIs.
            unsafe { LocalFree(self.0) };
        }
    }

    struct UserSid(Vec<usize>);

    impl UserSid {
        fn process() -> io::Result<Self> {
            let mut token = null_mut();
            // SAFETY: token is a live out parameter and the process handle is a pseudo-handle.
            if unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) } == 0 {
                return Err(io::Error::last_os_error());
            }
            let token = unsafe { OwnedHandle::from_raw_handle(token) };
            let mut length = 0;
            // SAFETY: the first call asks Win32 for the buffer size, the second owns that buffer.
            unsafe {
                GetTokenInformation(token.as_raw_handle(), TokenUser, null_mut(), 0, &mut length)
            };
            if length < size_of::<TOKEN_USER>() as u32 || length > 4096 {
                return Err(io::Error::last_os_error());
            }
            let mut user = vec![0usize; (length as usize).div_ceil(size_of::<usize>())];
            if unsafe {
                GetTokenInformation(
                    token.as_raw_handle(),
                    TokenUser,
                    user.as_mut_ptr().cast(),
                    length,
                    &mut length,
                )
            } == 0
            {
                return Err(io::Error::last_os_error());
            }
            Ok(Self(user))
        }

        fn sid(&self) -> PSID {
            // SAFETY: Win32 initialized a TOKEN_USER followed by its SID in this buffer.
            unsafe { (*self.0.as_ptr().cast::<TOKEN_USER>()).User.Sid }
        }
    }

    fn utf16(value: &OsStr) -> Vec<u16> {
        value.encode_wide().chain([0]).collect()
    }

    fn create_directory(path: &Path, user: &UserSid) -> io::Result<()> {
        let mut sid_text = null_mut();
        // SAFETY: the SID lives in user and Win32 allocates the result.
        if unsafe { ConvertSidToStringSidW(user.sid(), &mut sid_text) } == 0 {
            return Err(io::Error::last_os_error());
        }
        let _sid_text_allocation = LocalAllocation(sid_text.cast());
        // SAFETY: the returned string is NUL terminated.
        let sid_text = unsafe {
            let len = (0..).find(|&i| *sid_text.add(i) == 0).unwrap();
            String::from_utf16_lossy(std::slice::from_raw_parts(sid_text, len))
        };
        let sddl = format!("D:P(A;OICI;FA;;;{sid_text})(A;OICI;FA;;;SY)");
        let mut descriptor = null_mut();
        // SAFETY: the input is NUL terminated and the descriptor out parameter is valid.
        if unsafe {
            ConvertStringSecurityDescriptorToSecurityDescriptorW(
                utf16(OsStr::new(&sddl)).as_ptr(),
                1,
                &mut descriptor,
                null_mut(),
            )
        } == 0
        {
            return Err(io::Error::last_os_error());
        }
        let _descriptor_allocation = LocalAllocation(descriptor);
        let attributes = SECURITY_ATTRIBUTES {
            nLength: size_of::<SECURITY_ATTRIBUTES>() as u32,
            lpSecurityDescriptor: descriptor,
            bInheritHandle: 0,
        };
        // SAFETY: Win32 receives the path and security descriptor for this call only.
        if unsafe { CreateDirectoryW(utf16(path.as_os_str()).as_ptr(), &attributes) } != 0 {
            return Ok(());
        }
        let err = io::Error::last_os_error();
        if err.kind() == io::ErrorKind::AlreadyExists {
            Ok(())
        } else {
            Err(err)
        }
    }

    fn private(file: &File, user: &UserSid) -> io::Result<()> {
        let mut owner = null_mut();
        let mut acl: *mut ACL = null_mut();
        let mut descriptor = null_mut();
        // SAFETY: all out parameters live through the call and file owns its handle.
        let status = unsafe {
            GetSecurityInfo(
                file.as_raw_handle(),
                SE_FILE_OBJECT,
                DACL_SECURITY_INFORMATION | OWNER_SECURITY_INFORMATION,
                &mut owner,
                null_mut(),
                &mut acl,
                null_mut(),
                &mut descriptor,
            )
        };
        if status != ERROR_SUCCESS {
            return Err(io::Error::from_raw_os_error(status as i32));
        }
        let _descriptor_allocation = LocalAllocation(descriptor);
        let deny = || io::Error::from(io::ErrorKind::PermissionDenied);
        // A missing or NULL DACL is unrestricted. Unknown ACE types are refused.
        // SAFETY: owner and ACL point into the live security descriptor.
        unsafe {
            if owner.is_null()
                || acl.is_null()
                || windows_sys::Win32::Security::EqualSid(owner, user.sid()) == 0
            {
                return Err(deny());
            }
            for index in 0..u32::from((*acl).AceCount) {
                let mut ace = null_mut();
                if GetAce(acl, index, &mut ace) == 0 {
                    return Err(io::Error::last_os_error());
                }
                let ace = ace.cast::<ACCESS_ALLOWED_ACE>();
                if (*ace).Header.AceType != 0 {
                    return Err(deny());
                }
                let sid = addr_of!((*ace).SidStart).cast_mut().cast();
                if windows_sys::Win32::Security::EqualSid(sid, user.sid()) == 0
                    && IsWellKnownSid(sid, WinLocalSystemSid) == 0
                {
                    return Err(deny());
                }
            }
        }
        Ok(())
    }

    pub(super) fn ensure_directory(path: &Path) -> io::Result<()> {
        let user = UserSid::process()?;
        create_directory(path, &user)?;
        let file = OpenOptions::new()
            .access_mode(READ_CONTROL)
            .custom_flags(FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT)
            .open(path)?;
        let metadata = file.metadata()?;
        if !metadata.is_dir() || metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
            return Err(io::ErrorKind::PermissionDenied.into());
        }
        private(&file, &user)
    }

    pub(super) fn open_file(path: &Path) -> io::Result<File> {
        let file = OpenOptions::new()
            .access_mode(FILE_GENERIC_READ)
            .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT)
            .open(path)?;
        let metadata = file.metadata()?;
        if !metadata.is_file() || metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
            return Err(io::ErrorKind::PermissionDenied.into());
        }
        private(&file, &UserSid::process()?)?;
        Ok(file)
    }
}

#[cfg(not(any(unix, windows)))]
mod platform {
    use std::{fs::File, io, path::Path};

    pub(super) fn ensure_directory(_: &Path) -> io::Result<()> {
        Err(io::ErrorKind::Unsupported.into())
    }

    pub(super) fn open_file(_: &Path) -> io::Result<File> {
        Err(io::ErrorKind::Unsupported.into())
    }
}
