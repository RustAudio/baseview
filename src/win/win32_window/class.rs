use std::ffi::OsStr;
use std::mem::MaybeUninit;
use std::os::windows::ffi::OsStrExt;
use std::ptr::null_mut;
use winapi::shared::minwindef::ATOM;
use winapi::um::combaseapi::CoCreateGuid;
use winapi::um::winuser::{
    LoadCursorW, RegisterClassW, UnregisterClassW, CS_OWNDC, IDC_ARROW, WNDCLASSW,
};

pub struct WndClass(ATOM);

impl WndClass {
    // TODO: manage error
    pub fn register() -> Self {
        let proc = crate::win::proc::wnd_proc;
        // We generate a unique name for the new window class to prevent name collisions
        let class_name_str = format!("Baseview-{}", generate_guid());
        let mut class_name: Vec<u16> = OsStr::new(&class_name_str).encode_wide().collect();
        class_name.push(0);

        let wnd_class = WNDCLASSW {
            style: CS_OWNDC,
            lpfnWndProc: Some(proc),
            hInstance: null_mut(),
            lpszClassName: class_name.as_ptr(),
            cbClsExtra: 0,
            cbWndExtra: 0,
            hIcon: null_mut(),
            hCursor: unsafe { LoadCursorW(null_mut(), IDC_ARROW) },
            hbrBackground: null_mut(),
            lpszMenuName: null_mut(),
        };

        Self(unsafe { RegisterClassW(&wnd_class) })
    }

    pub fn atom(&self) -> ATOM {
        self.0
    }
}

impl Drop for WndClass {
    fn drop(&mut self) {
        unsafe {
            UnregisterClassW(self.0 as _, null_mut());
        }
    }
}

fn generate_guid() -> String {
    let mut guid = MaybeUninit::zeroed();
    // SAFETY: the given output pointer is valid
    unsafe { CoCreateGuid(guid.as_mut_ptr()) };
    // SAFETY: CoCreateGuid should have initialized the GUID.
    // In the worst case, a GUID is all just numbers, so it's still safe to read even when zeroed.
    let guid = unsafe { guid.assume_init_ref() };
    format!(
        "{:0X}-{:0X}-{:0X}-{:0X}{:0X}-{:0X}{:0X}{:0X}{:0X}{:0X}{:0X}\0",
        guid.Data1,
        guid.Data2,
        guid.Data3,
        guid.Data4[0],
        guid.Data4[1],
        guid.Data4[2],
        guid.Data4[3],
        guid.Data4[4],
        guid.Data4[5],
        guid.Data4[6],
        guid.Data4[7]
    )
}
