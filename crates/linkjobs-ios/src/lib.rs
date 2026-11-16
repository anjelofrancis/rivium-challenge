//! C ABI for the iOS app. See `include/linkjobs.h`.
//!
//! Rivium ships a JNI bridge for Android but nothing for Apple platforms, so this crate
//! wraps the same `rivium_host::embedded::Host` the JNI bridge uses in a few `extern "C"`
//! functions that Swift can call through the `LinkJobsCore` module.

use std::{
    ffi::{CStr, CString, c_char},
    panic::{AssertUnwindSafe, catch_unwind},
    sync::OnceLock,
    time::Duration,
};

use linkjobs_core::{DEFAULT_PORT, LinkJobs};
use rivium_host::{App, Code, embedded::Host};

const PANICKED: i32 = -99;

fn host() -> &'static Host {
    static HOST: OnceLock<Host> = OnceLock::new();
    HOST.get_or_init(Host::new::<LinkJobs>)
}

fn guarded(f: impl FnOnce() -> i32) -> i32 {
    catch_unwind(AssertUnwindSafe(f)).unwrap_or(PANICKED)
}

fn into_c(s: String) -> *mut c_char {
    CString::new(s.replace('\0', " "))
        .map(CString::into_raw)
        .unwrap_or(std::ptr::null_mut())
}

/// # Safety
/// `root` must be null or a valid NUL-terminated string that stays alive for the call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn linkjobs_start(root: *const c_char) -> i32 {
    if root.is_null() {
        return Code::Usage.ffi();
    }
    // SAFETY: caller guarantees `root` is a valid C string (checked non-null above).
    let root = unsafe { CStr::from_ptr(root) }
        .to_string_lossy()
        .into_owned();
    guarded(|| host().start(&["--root".to_owned(), root]).ffi())
}

#[unsafe(no_mangle)]
pub extern "C" fn linkjobs_stop(timeout_ms: u64) -> i32 {
    guarded(|| host().stop(Duration::from_millis(timeout_ms)).ffi())
}

#[unsafe(no_mangle)]
pub extern "C" fn linkjobs_status() -> i32 {
    guarded(|| host().status() as i32)
}

#[unsafe(no_mangle)]
pub extern "C" fn linkjobs_port() -> u16 {
    DEFAULT_PORT
}

#[unsafe(no_mangle)]
pub extern "C" fn linkjobs_last_error() -> *mut c_char {
    catch_unwind(|| host().last_error())
        .ok()
        .flatten()
        .map_or(std::ptr::null_mut(), into_c)
}

#[unsafe(no_mangle)]
pub extern "C" fn linkjobs_version() -> *mut c_char {
    into_c(LinkJobs::VERSION.to_owned())
}

/// # Safety
/// `s` must be null or a pointer returned by this library that has not been freed yet.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn linkjobs_string_free(s: *mut c_char) {
    if !s.is_null() {
        // SAFETY: `s` came from `CString::into_raw` in this library (caller contract).
        drop(unsafe { CString::from_raw(s) });
    }
}
