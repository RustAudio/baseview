use std::ptr::null_mut;
use windows_core::Error;
use windows_sys::Win32::Foundation::HINSTANCE;
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;

#[derive(Copy, Clone, PartialEq)]
pub struct HInstance(HINSTANCE);

// SAFETY: This is actually a pointer to the memory image of the executable file. It is guaranteed
// to be valid for the process's lifetime.
// This getting invalidated would imply our own executable has been unloaded already. At that point,
// pointer invalidation would the least of our concerns anyway.
unsafe impl Send for HInstance {}
// SAFETY: same as above
unsafe impl Sync for HInstance {}

impl HInstance {
    pub fn get() -> Self {
        let result = unsafe { GetModuleHandleW(null_mut()) };
        if result.is_null() {
            panic!(
                "Failed to get HInstance pointer: GetModuleHandleW failed: {}",
                Error::from_win32()
            );
        }

        Self(result)
    }

    #[inline]
    pub fn as_raw(&self) -> HINSTANCE {
        self.0
    }
}
