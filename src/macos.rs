use crate::{Error, LinuxOptions};
use objc2_core_foundation::CFString;
use objc2_io_kit::{
    IOPMAssertionCreateWithName, IOPMAssertionID, IOPMAssertionRelease, kIOPMAssertionLevelOn,
    kIOReturnSuccess,
};

const ASSERTION_TYPE_NO_DISPLAY_SLEEP: &str = "NoDisplaySleepAssertion";

pub struct Inner {
    id: IOPMAssertionID,
    active: bool,
}

pub fn is_supported() -> bool {
    true
}

pub fn acquire(reason: &str, _linux: LinuxOptions) -> Result<Inner, Error> {
    let assertion_type = CFString::from_static_str(ASSERTION_TYPE_NO_DISPLAY_SLEEP);
    let assertion_name = CFString::from_str(reason);

    let mut id: IOPMAssertionID = 0;
    let rc = unsafe {
        IOPMAssertionCreateWithName(
            Some(&assertion_type),
            kIOPMAssertionLevelOn,
            Some(&assertion_name),
            &mut id as *mut IOPMAssertionID,
        )
    };

    if rc != kIOReturnSuccess {
        return Err(Error::Os(format!(
            "IOPMAssertionCreateWithName failed (IOReturn={rc})"
        )));
    }

    Ok(Inner { id, active: true })
}

pub fn release(inner: &mut Inner) {
    if !inner.active {
        return;
    }
    let _ = IOPMAssertionRelease(inner.id);
    inner.active = false;
}
