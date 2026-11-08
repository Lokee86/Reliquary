use std::sync::atomic::{AtomicBool, Ordering};

static INTERRUPTED: AtomicBool = AtomicBool::new(false);

pub(crate) struct InterruptGuard;

impl InterruptGuard {
    pub fn install() -> std::io::Result<Self> {
        INTERRUPTED.store(false, Ordering::Release);
        platform::install()?;
        Ok(Self)
    }

    pub fn interrupted(&self) -> bool {
        INTERRUPTED.load(Ordering::Acquire)
    }
}

impl Drop for InterruptGuard {
    fn drop(&mut self) {
        platform::remove();
    }
}

#[cfg(windows)]
mod platform {
    use super::*;

    #[link(name = "Kernel32")]
    unsafe extern "system" {
        fn SetConsoleCtrlHandler(handler: extern "system" fn(u32) -> i32, add: i32) -> i32;
    }

    extern "system" fn interrupt(event: u32) -> i32 {
        if matches!(event, 0 | 1) {
            INTERRUPTED.store(true, Ordering::Release);
            1
        } else {
            0
        }
    }

    pub(super) fn install() -> std::io::Result<()> {
        // This static callback has the Win32 handler signature and only sets a flag.
        if unsafe { SetConsoleCtrlHandler(interrupt, 1) } != 0 {
            Ok(())
        } else {
            Err(std::io::Error::last_os_error())
        }
    }

    pub(super) fn remove() {
        // Remove the same callback registered above.
        unsafe { SetConsoleCtrlHandler(interrupt, 0) };
    }
}

#[cfg(unix)]
mod platform {
    use super::*;
    use std::sync::atomic::AtomicUsize;

    static PREVIOUS: AtomicUsize = AtomicUsize::new(0);
    const SIGINT: i32 = 2;

    unsafe extern "C" {
        fn signal(number: i32, handler: usize) -> usize;
    }

    extern "C" fn interrupt(_: i32) {
        INTERRUPTED.store(true, Ordering::Release);
    }

    pub(super) fn install() -> std::io::Result<()> {
        // The signal handler is static and only updates a lock-free atomic.
        let previous = unsafe { signal(SIGINT, interrupt as *const () as usize) };
        if previous == usize::MAX {
            Err(std::io::Error::last_os_error())
        } else {
            PREVIOUS.store(previous, Ordering::Release);
            Ok(())
        }
    }

    pub(super) fn remove() {
        // Restore the signal handler that was active before this command.
        unsafe { signal(SIGINT, PREVIOUS.load(Ordering::Acquire)) };
    }
}

#[cfg(not(any(windows, unix)))]
mod platform {
    pub(super) fn install() -> std::io::Result<()> {
        Err(std::io::ErrorKind::Unsupported.into())
    }

    pub(super) fn remove() {}
}
