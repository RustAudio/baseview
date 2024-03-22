use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;

pub fn to_wstr(str: &str) -> Vec<u16> {
    let mut title: Vec<u16> = OsStr::new(str).encode_wide().collect();
    title.push(0);
    title
}
