#[cfg(target_os = "macos")]
use crate::macos as platform;
#[cfg(target_os = "windows")]
use crate::win as platform;
#[cfg(target_os = "linux")]
use crate::x11 as platform;

#[cfg(target_os = "macos")]
pub fn copy_to_clipboard(data: String) {
    platform::copy_to_clipboard(data)
}
