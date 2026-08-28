use crate::wrappers::egl::{sys, Egl};
use std::ffi::CStr;
use std::fmt::Debug;

pub struct Extensions {
    string: Option<&'static CStr>,
}

impl Debug for Extensions {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self.string {
            Some(s) => f.write_str(&s.to_string_lossy()),
            None => f.write_str("<none>"),
        }
    }
}

impl Extensions {
    pub(super) fn new(egl: &Egl) -> Extensions {
        Self { string: egl.query_client_extensions_inner() }
    }

    pub fn supports(&self, extension_id: &[u8]) -> bool {
        let Some(string) = self.string else { return false };
        string.to_bytes().split(|b| *b == b' ').any(|s| s == extension_id)
    }
}

impl Egl {
    fn query_client_extensions_inner(&self) -> Option<&'static CStr> {
        let result =
            unsafe { (self.inner.functions.eglQueryString)(sys::NO_DISPLAY, sys::EXTENSIONS) };
        if result.is_null() {
            None
        } else {
            unsafe { Some(CStr::from_ptr(result)) }
        }
    }
}
