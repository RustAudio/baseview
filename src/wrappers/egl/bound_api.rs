use super::*;

pub struct BoundApi {
    egl: Egl,
    previous_api: Option<sys::Enum>,
}

impl BoundApi {
    pub(super) fn new(egl: &Egl) -> Result<Self, EglError> {
        let previous_api = egl.query_api();
        egl.bind_api(sys::OPENGL_API)?;

        Ok(Self { previous_api, egl: egl.clone() })
    }
}

impl Drop for BoundApi {
    fn drop(&mut self) {
        let Some(previous_api) = self.previous_api else { return };

        if let Err(e) = self.egl.bind_api(previous_api) {
            crate::warn!("Failed to restore EGL api: {}", e);
        }
    }
}

impl Egl {
    fn query_api(&self) -> Option<sys::Enum> {
        let result = unsafe { (self.inner.functions.eglQueryAPI)() };
        if result == sys::ENUM_NONE {
            None
        } else {
            Some(result)
        }
    }

    fn bind_api(&self, api: sys::Enum) -> Result<(), EglError> {
        let result = unsafe { (self.inner.functions.eglBindAPI)(api) };
        if result == sys::FALSE {
            Err(EglError::from_last_error(self))
        } else {
            Ok(())
        }
    }
}
