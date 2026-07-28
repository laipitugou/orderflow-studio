//! Prevents Windows from sleeping while the application is running.

/// Windows power management for the application.
#[cfg(target_os = "windows")]
pub mod windows_power {
    use std::sync::atomic::{AtomicBool, Ordering};
    use windows_sys::Win32::System::Power::{
        ES_CONTINUOUS, ES_SYSTEM_REQUIRED, SetThreadExecutionState,
    };

    static POWER_GUARD_ACTIVE: AtomicBool = AtomicBool::new(false);

    fn set_system_required(active: bool) -> bool {
        let flags = ES_CONTINUOUS | if active { ES_SYSTEM_REQUIRED } else { 0 };
        // SAFETY: `flags` is a valid EXECUTION_STATE bitmask and the call does
        // not retain pointers or references.
        let previous_state = unsafe { SetThreadExecutionState(flags) };
        if previous_state == 0 {
            log::error!(
                "WINDOWS ExecutionState | active={active} result=error error={}",
                std::io::Error::last_os_error()
            );
            false
        } else {
            log::info!("WINDOWS ExecutionState | active={active} result=ok");
            true
        }
    }

    /// Check if the power guard is active.
    pub fn is_active() -> bool {
        POWER_GUARD_ACTIVE.load(Ordering::Relaxed)
    }

    pub fn init() {
        if !is_active() && set_system_required(true) {
            POWER_GUARD_ACTIVE.store(true, Ordering::Relaxed);
        }
    }

    /// Restore the normal execution state during application shutdown.
    pub fn cleanup() {
        if is_active() && set_system_required(false) {
            POWER_GUARD_ACTIVE.store(false, Ordering::Relaxed);
        }
    }
}

/// Cross-platform power guard that does nothing on non-Windows platforms.
#[cfg(not(target_os = "windows"))]
pub mod windows_power {
    pub fn cleanup() {}
}
