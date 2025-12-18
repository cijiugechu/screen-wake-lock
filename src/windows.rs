use crate::WakeLockResult;
use windows::Win32::Foundation::{CloseHandle, HANDLE};
use windows::Win32::System::Power::{
    POWER_REQUEST_TYPE, PowerClearRequest, PowerCreateRequest, PowerSetRequest,
};
use windows::Win32::System::Threading::{
    POWER_REQUEST_CONTEXT_FLAGS, REASON_CONTEXT, REASON_CONTEXT_0,
};
use windows::core::PWSTR;

// Values from Win32 headers.
const POWER_REQUEST_CONTEXT_VERSION: u32 = 0;
const POWER_REQUEST_CONTEXT_SIMPLE_STRING: u32 = 0x0000_0001;

// `POWER_REQUEST_TYPE` is a C enum; in `windows` it's projected as a tuple struct.
const POWER_REQUEST_DISPLAY_REQUIRED: POWER_REQUEST_TYPE = POWER_REQUEST_TYPE(0);
const POWER_REQUEST_SYSTEM_REQUIRED: POWER_REQUEST_TYPE = POWER_REQUEST_TYPE(1);

pub struct PlatformWakeLock {
    handle: HANDLE,
    // Keep the buffer alive during the `PowerCreateRequest` call.
    _reason_wide: Vec<u16>,
}

impl PlatformWakeLock {
    pub fn acquire(reason: &str) -> WakeLockResult<Self> {
        let mut reason_wide: Vec<u16> = reason.encode_utf16().collect();
        reason_wide.push(0);

        let ctx = REASON_CONTEXT {
            Version: POWER_REQUEST_CONTEXT_VERSION,
            Flags: POWER_REQUEST_CONTEXT_FLAGS(POWER_REQUEST_CONTEXT_SIMPLE_STRING),
            Reason: REASON_CONTEXT_0 {
                SimpleReasonString: PWSTR(reason_wide.as_mut_ptr()),
            },
        };

        let handle = unsafe { PowerCreateRequest(&ctx) }?;

        // Docs recommend pairing DisplayRequired with SystemRequired.
        if let Err(e) = (|| -> WakeLockResult<()> {
            unsafe { PowerSetRequest(handle, POWER_REQUEST_SYSTEM_REQUIRED)? };
            unsafe { PowerSetRequest(handle, POWER_REQUEST_DISPLAY_REQUIRED)? };
            Ok(())
        })() {
            unsafe {
                let _ = CloseHandle(handle);
            }
            return Err(e);
        }

        Ok(Self {
            handle,
            _reason_wide: reason_wide,
        })
    }

    pub fn release(self) -> WakeLockResult<()> {
        unsafe {
            let _ = PowerClearRequest(self.handle, POWER_REQUEST_DISPLAY_REQUIRED);
            let _ = PowerClearRequest(self.handle, POWER_REQUEST_SYSTEM_REQUIRED);
            CloseHandle(self.handle)?;
        }
        Ok(())
    }
}
