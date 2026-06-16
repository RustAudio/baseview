#[cfg(target_os = "macos")]
#[path = "platform/macos/mod.rs"]
mod platform;
#[cfg(target_os = "linux")]
mod x11;
#[cfg(target_os = "linux")]
pub use x11::*;
#[cfg(target_os = "windows")]
#[path = "platform/win/mod.rs"]
mod platform;
