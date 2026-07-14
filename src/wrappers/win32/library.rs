use std::ffi::c_void;
use std::ptr::NonNull;
use windows_core::{Error, PCSTR};
use windows_sys::Win32::Foundation::FreeLibrary;
use windows_sys::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryA};

pub struct LibraryModule(NonNull<c_void>);

impl LibraryModule {
    pub unsafe fn load(module_name: PCSTR) -> Result<Self, Error> {
        let library = unsafe { LoadLibraryA(module_name.as_ptr()) };
        let Some(library) = NonNull::new(library) else { return Err(Error::from_thread()) };

        Ok(Self(library))
    }

    pub unsafe fn get_proc_address(&self, name: PCSTR) -> Option<*const c_void> {
        let addr = unsafe { GetProcAddress(self.0.as_ptr(), name.as_ptr()) };

        addr.map(|f| f as _)
    }
}

impl Drop for LibraryModule {
    fn drop(&mut self) {
        unsafe { FreeLibrary(self.0.as_ptr()) };
    }
}
