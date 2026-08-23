use crate::platform::gl::CreationFailedError;
use libloading::Library;
use std::ffi::{c_void, CStr};
use std::rc::Rc;

mod bound_api;
mod config;
mod context;
mod display;
mod error;
mod extensions;
mod surface;
mod sys;

use sys::Functions;

use crate::wrappers::egl::extensions::Extensions;
pub use bound_api::BoundApi;
pub use config::EglConfig;
pub use context::EglContext;
pub use display::{EglDisplay, EglVersion};
pub use error::EglError;
pub use surface::EglSurface;
pub use sys::MissingSymbolError;

struct EglInner {
    _library: Library,
    functions: Functions,
}

#[derive(Clone)]
pub struct Egl {
    inner: Rc<EglInner>,
}

impl Egl {
    pub fn open() -> Result<Self, CreationFailedError> {
        let library =
            unsafe { Library::new("libEGL.so.1").or_else(|_| Library::new("libEGL.so")) }?;

        let functions = unsafe { Functions::load_from(&library)? };

        Ok(Self { inner: Rc::new(EglInner { _library: library, functions }) })
    }

    pub fn with_opengl<T>(&self, handler: impl FnOnce(&BoundApi) -> T) -> Result<T, EglError> {
        let api = BoundApi::new(self)?;
        Ok(handler(&api))
    }

    pub fn query_client_extensions(&self) -> Extensions {
        Extensions::new(self)
    }

    pub fn get_proc_address(&self, proc_name: &CStr) -> *const c_void {
        unsafe { (self.inner.functions.eglGetProcAddress)(proc_name.as_ptr()) }
    }
}
