pub mod h_instance;
mod rect;
pub mod uuid;
pub mod window;

pub use rect::Rect;

use windows_core::{Error, Result, HRESULT};
use windows_sys::Win32::Foundation::{S_FALSE, S_OK};
use windows_sys::Win32::System::Ole::OleInitialize;

pub fn ole_initialize() -> Result<()> {
    // SAFETY: this is always safe to call with NULL
    match unsafe { OleInitialize(core::ptr::null_mut()) } {
        S_OK | S_FALSE => Ok(()),
        result => Err(Error::new(HRESULT(result), "OLE initialization failed")),
    }
}
