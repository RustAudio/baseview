mod win;

pub use win::*;

use std::ffi::c_void;

mod x11;

mod macos;
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
    x11::run(options);
}
