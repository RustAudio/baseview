// todo: will deal with conditional compilation/visibility later,
// todo: we still have to choose how to organize the code
// todo: for now I need this to be able to check and compile
// todo: We should consider doing it as winit does it
#[cfg(target_os = "windows")]
mod win;
#[cfg(target_os = "windows")]
pub use win::*;

use std::ffi::c_void;

#[cfg(target_os = "linux")]
mod x11;

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
pub use macos::Window;

pub enum Parent {
    None,
    AsIfParented,
    WithParent(*mut c_void),
}

pub struct WindowOpenOptions<'a> {
    pub title: &'a str,

    pub width: usize,
    pub height: usize,

    pub parent: Parent,
}

pub fn run(options: WindowOpenOptions) {
    #[cfg(target_os = "linux")]
    x11::run(options);
}
