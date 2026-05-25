use crate::PhySize;
use windows_core::{Error, Result};
use windows_sys::Win32::Foundation::RECT;
use windows_sys::Win32::UI::WindowsAndMessaging::AdjustWindowRectEx;

#[derive(Copy, Clone)]
pub struct Rect(pub RECT);

impl Rect {
    pub fn client_area_to_nc_area(mut self, style: u32) -> Result<Self> {
        let result = unsafe { AdjustWindowRectEx(&mut self.0, style, 0, 0) };
        if result == 0 {
            return Err(Error::from_win32());
        }

        Ok(self)
    }

    pub fn size(&self) -> PhySize {
        PhySize {
            width: self.0.right.abs_diff(self.0.left),
            height: self.0.top.abs_diff(self.0.bottom),
        }
    }
}

impl From<PhySize> for Rect {
    fn from(size: PhySize) -> Self {
        Self(RECT {
            left: 0,
            top: 0,
            right: size.width.try_into().unwrap_or(i32::MAX),
            bottom: size.height.try_into().unwrap_or(i32::MAX),
        })
    }
}
