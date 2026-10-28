use crate::MasterKeyError;

#[cfg(windows)]
pub(crate) fn fill_random(bytes: &mut [u8]) -> Result<(), MasterKeyError> {
    use std::ptr::null_mut;
    const BCRYPT_USE_SYSTEM_PREFERRED_RNG: u32 = 0x00000002;
    #[link(name = "Bcrypt")]
    unsafe extern "system" {
        fn BCryptGenRandom(
            handle: *mut std::ffi::c_void,
            buffer: *mut u8,
            len: u32,
            flags: u32,
        ) -> i32;
    }
    let len = u32::try_from(bytes.len()).map_err(|_| MasterKeyError::EntropyUnavailable)?;
    let status = unsafe {
        BCryptGenRandom(
            null_mut(),
            bytes.as_mut_ptr(),
            len,
            BCRYPT_USE_SYSTEM_PREFERRED_RNG,
        )
    };
    (status >= 0)
        .then_some(())
        .ok_or(MasterKeyError::EntropyUnavailable)
}

#[cfg(unix)]
pub(crate) fn fill_random(bytes: &mut [u8]) -> Result<(), MasterKeyError> {
    use std::io::Read;
    std::fs::File::open("/dev/urandom")?.read_exact(bytes)?;
    Ok(())
}

#[cfg(not(any(windows, unix)))]
pub(crate) fn fill_random(_bytes: &mut [u8]) -> Result<(), MasterKeyError> {
    Err(MasterKeyError::EntropyUnavailable)
}
