use crate::platform::gl::CreationFailedError;
use libloading::Library;
use std::sync::Arc;

mod bound_api;
mod error;
mod extensions;
mod sys;

use bound_api::BoundApi;
use sys::Functions;

use crate::wrappers::egl::extensions::Extensions;
pub use error::EglError;
pub use sys::MissingSymbolError;

struct EglInner {
    _library: Library,
    functions: Functions,
}

#[derive(Clone)]
pub struct Egl {
    inner: Arc<EglInner>,
}

impl Egl {
    pub fn open() -> Result<Self, CreationFailedError> {
        let library =
            unsafe { Library::new("libEGL.so.1").or_else(|_| Library::new("libEGL.so")) }?;

        let functions = unsafe { Functions::load_from(&library)? };

        Ok(Self { inner: Arc::new(EglInner { _library: library, functions }) })
    }

    pub fn with_opengl<T>(&self, handler: impl FnOnce(&BoundApi) -> T) -> Result<T, EglError> {
        let api = BoundApi::new(self)?;
        Ok(handler(&api))
    }

    pub fn query_client_extensions(&self) -> Extensions {
        Extensions::new(self)
    }
}
