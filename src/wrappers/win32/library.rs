use std::cell::LazyCell;
use std::ffi::{c_void, CStr};
use std::ops::Deref;
use std::ptr::NonNull;
use windows_core::Error;
use windows_sys::Win32::Foundation::FreeLibrary;
use windows_sys::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryA};

/// # Safety
///
/// Implementations must ensure that the module with name `MODULE_NAME` is safe to load.
pub unsafe trait Module: Clone + Sized {
    const MODULE_NAME: &'static CStr;
    fn load(library: &RawLibrary) -> Self;
}

pub struct LibraryModule<M> {
    _library: RawLibrary,
    module: M,
}

pub type LazyLibraryModule<M> = LazyCell<Option<LibraryModule<M>>>;

impl<M: Module> LibraryModule<M> {
    pub fn load() -> Result<Self, Error> {
        let library = unsafe { RawLibrary::load(M::MODULE_NAME)? };
        Ok(Self { module: M::load(&library), _library: library })
    }

    pub fn lazy() -> LazyLibraryModule<M> {
        LazyCell::new(|| match Self::load() {
            Ok(module) => Some(module),
            Err(err) => {
                crate::warn!(
                    "Error loading module '{}': {}",
                    M::MODULE_NAME.to_string_lossy(),
                    err
                );

                None
            }
        })
    }
}

impl<M: Module> Clone for LibraryModule<M> {
    fn clone(&self) -> Self {
        let library = unsafe { RawLibrary::load(M::MODULE_NAME) };

        // PANIC: This should not be able to happen, since we already loaded it once and it's still loaded in Clone
        let library = match library {
            Ok(library) => library,
            Err(e) => unreachable!("Failed to load module: {}", e),
        };

        Self { _library: library, module: self.module.clone() }
    }
}

impl<M> Deref for LibraryModule<M> {
    type Target = M;
    fn deref(&self) -> &M {
        &self.module
    }
}

pub struct RawLibrary(NonNull<c_void>);

impl RawLibrary {
    pub unsafe fn load(module_name: &CStr) -> Result<Self, Error> {
        let library = unsafe { LoadLibraryA(module_name.as_ptr().cast()) };
        let Some(library) = NonNull::new(library) else { return Err(Error::from_thread()) };

        Ok(Self(library))
    }

    /// # Safety
    ///
    /// T *must* be a function pointer type, and must match the given `name`.
    pub unsafe fn get<T: Copy + Sized>(&self, name: &CStr) -> Option<T> {
        let addr = unsafe { GetProcAddress(self.0.as_ptr(), name.as_ptr().cast()) }?;

        Some(core::mem::transmute_copy(&addr))
    }
}

impl Drop for RawLibrary {
    fn drop(&mut self) {
        unsafe { FreeLibrary(self.0.as_ptr()) };
    }
}
