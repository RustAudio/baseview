use std::ffi::c_void;

pub enum Parent {
    None,
    AsIfParented,
    WithParent(*mut c_void)
}

pub struct WindowOpenOptions<'a> {
    pub title: &'a str,

    pub width: usize,
    pub height: usize,

    pub parent: Parent
}
