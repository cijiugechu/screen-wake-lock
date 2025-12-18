//! screen-wake-lock: minimal cross-platform "keep screen awake" guard.
//!
//! Usage:
//! ```no_run
//! let lock = screen_wake_lock::ScreenWakeLock::acquire("Playing video")?;
//! // keep running...
//! drop(lock); // screen can sleep again
//! # Ok::<(), screen_wake_lock::Error>(())
//! ```

use cfg_if::cfg_if;

cfg_if! {
    if #[cfg(target_os = "windows")] {
        mod windows;
        use windows as sys;
    } else if #[cfg(target_os = "macos")] {
        mod macos;
        use macos as sys;
    } else if #[cfg(target_os = "linux")] {
        mod linux;
        use linux as sys;
    } else {
        compile_error!("screen-wake-lock only supports Windows, macOS, and Linux.");
    }
}

/// Error type for wake lock acquisition.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// Generic OS error (e.g. Win32 GetLastError).
    #[error("OS error: {0}")]
    Os(String),
    /// D-Bus error (Linux).
    #[error("D-Bus error: {0}")]
    Dbus(String),
    /// Not supported in the current environment (Linux without a session bus/service).
    #[error("Unsupported: {0}")]
    Unsupported(String),
}

/// Options for Linux (ignored on Windows/macOS).
#[derive(Clone, Debug)]
pub struct LinuxOptions {
    /// D-Bus "application name" / app_id (often reverse-DNS). If None, a default is used.
    pub application_id: Option<String>,
    /// Human readable reason. If None, the `reason` passed to `acquire*` is used.
    pub reason: Option<String>,
}

impl Default for LinuxOptions {
    fn default() -> Self {
        Self {
            application_id: None,
            reason: None,
        }
    }
}

/// Guard that keeps the **display** from idling/sleeping while alive.
pub struct ScreenWakeLock {
    inner: sys::Inner,
}

impl ScreenWakeLock {
    /// Acquire a screen wake lock with a reason string.
    pub fn acquire(reason: impl Into<String>) -> Result<Self, Error> {
        Self::acquire_with_linux_options(reason, LinuxOptions::default())
    }

    /// Acquire, with extra Linux-specific options (safe to call on all platforms).
    pub fn acquire_with_linux_options(
        reason: impl Into<String>,
        linux: LinuxOptions,
    ) -> Result<Self, Error> {
        let reason = reason.into();
        let inner = sys::acquire(&reason, linux)?;
        Ok(Self { inner })
    }

    /// Best-effort check (Linux: checks for a usable inhibitor service).
    pub fn is_supported() -> bool {
        sys::is_supported()
    }

    /// Explicitly release early (also happens automatically on Drop).
    pub fn release(self) {
        drop(self);
    }
}

impl Drop for ScreenWakeLock {
    fn drop(&mut self) {
        sys::release(&mut self.inner);
    }
}
