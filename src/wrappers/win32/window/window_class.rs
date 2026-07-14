use crate::wrappers::win32::h_instance::HInstance;
use crate::wrappers::win32::uuid::Uuid;
use std::num::NonZeroU16;
use std::ptr::null_mut;
use std::sync::Arc;
use windows_core::{Error, Result, HSTRING};
use windows_sys::core::PCWSTR;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    LoadCursorW, RegisterClassW, UnregisterClassW, CS_OWNDC, IDC_ARROW, WNDCLASSW, WNDPROC,
};

#[derive(Clone)]
pub struct RegisteredClass(Arc<RegisteredClassInner>);

impl RegisteredClass {
    pub fn register_new(instance: HInstance, wnd_proc: WNDPROC) -> Result<Self> {
        let class_name = format!("Baseview-{}", Uuid::new());
        let class_name = HSTRING::from(&class_name);

        let class_info = WNDCLASSW {
            lpfnWndProc: wnd_proc,
            hInstance: instance.as_raw(),
            lpszClassName: class_name.as_ptr(),

            style: CS_OWNDC,
            cbClsExtra: 0,
            cbWndExtra: 0,
            hIcon: null_mut(),                                      // Default icon
            hCursor: unsafe { LoadCursorW(null_mut(), IDC_ARROW) }, // Arrow cursor
            hbrBackground: null_mut(),                              // No default background
            lpszMenuName: null_mut(),                               // No default menu
        };

        let class_atom = unsafe { RegisterClassW(&class_info) };

        let Some(class_atom) = NonZeroU16::new(class_atom) else {
            return Err(Error::from_thread());
        };

        Ok(Self(Arc::new(RegisteredClassInner(class_atom, instance))))
    }

    #[inline]
    pub fn as_atom_ptr(&self) -> PCWSTR {
        self.0.as_atom_ptr()
    }
}

struct RegisteredClassInner(NonZeroU16, HInstance);

impl RegisteredClassInner {
    // See:
    // https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-createwindowexw
    // https://networkdls.com/Win32Ref/MAKEINTATOM.html
    // https://learn.microsoft.com/en-us/windows/win32/winprog/windows-data-types
    #[inline]
    pub fn as_atom_ptr(&self) -> PCWSTR {
        self.0.get() as u32 as PCWSTR
    }
}

impl Drop for RegisteredClassInner {
    fn drop(&mut self) {
        // Ignore errors from this, at worst this is a small memory leak.
        let _ = unsafe { UnregisterClassW(self.as_atom_ptr(), self.1.as_raw()) };
    }
}
