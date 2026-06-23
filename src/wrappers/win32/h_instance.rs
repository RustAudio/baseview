use std::ffi::c_void;
use std::num::NonZeroIsize;
use std::ptr::NonNull;
use windows_sys::Win32::Foundation::HINSTANCE;
use windows_sys::Win32::System::SystemServices::IMAGE_DOS_HEADER;

#[derive(Copy, Clone, PartialEq)]
pub struct HInstance(NonNull<c_void>);

// SAFETY: This is actually a pointer to the memory image of the executable file. It is guaranteed
// to be valid for the process's lifetime.
// This getting invalidated would imply our own executable has been unloaded already. At that point,
// pointer invalidation would the least of our concerns anyway.
unsafe impl Send for HInstance {}
// SAFETY: same as above
unsafe impl Sync for HInstance {}

impl HInstance {
    pub fn get_from_dll() -> Self {
        extern "C" {
            static __ImageBase: IMAGE_DOS_HEADER;
        }

        unsafe { Self(NonNull::from(&__ImageBase).cast()) }
    }

    #[inline]
    pub fn as_raw(&self) -> HINSTANCE {
        self.0.as_ptr()
    }

    pub fn addr(&self) -> NonZeroIsize {
        match NonZeroIsize::new(self.0.as_ptr() as isize) {
            Some(addr) => addr,
            None => unreachable!(),
        }
    }
}
