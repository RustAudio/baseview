use crate::WindowSize;
pub use dpi::*;

/// A size represented in the platform's native pixels.
///
/// This size is represented in physical pixels on Windows and Linux, and in logical pixels on macOS.
///
/// # Platform compatibility notes
///
/// On Windows, this *always* represents *actual* pixels, *NOT* the "physical pixels" window
/// implementations think they are using when using the Win32 APIs. TODO: finish sentence
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct HostSize {
    pub width: u32,
    pub height: u32,
}
