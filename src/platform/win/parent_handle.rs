use crate::wrappers::win32::window::HWnd;
use raw_window_handle::{HandleError, HasWindowHandle, RawWindowHandle};
use std::fmt::{Display, Formatter};
use std::ptr::NonNull;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParentWindowHandle {
    handle: HWnd,
}

// SAFETY: ParentWindowHandle does not actually expose any thread-unsafe operation
unsafe impl Send for ParentWindowHandle {}
// SAFETY: ParentWindowHandle does not actually expose any thread-unsafe operation
unsafe impl Sync for ParentWindowHandle {}

impl ParentWindowHandle {
    pub fn extract(parent: &impl HasWindowHandle) -> Result<Self, ParentWindowHandleError> {
        let parent = match parent.window_handle()?.as_raw() {
            RawWindowHandle::Win32(h) => h.hwnd,
            h => return Err(ParentWindowHandleError::UnsupportedWindowHandleType(h)),
        };

        let Some(parent) = NonNull::new(parent.get() as _) else { unreachable!() };

        Ok(Self { handle: unsafe { HWnd::from_raw(parent) } })
    }

    pub(crate) fn hwnd(&self) -> HWnd {
        self.handle
    }
}

pub enum ParentWindowHandleError {
    HandleError(HandleError),
    UnsupportedWindowHandleType(RawWindowHandle),
}

impl From<HandleError> for ParentWindowHandleError {
    fn from(err: HandleError) -> Self {
        Self::HandleError(err)
    }
}

impl Display for ParentWindowHandleError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            ParentWindowHandleError::HandleError(e) => Display::fmt(e, f),
            ParentWindowHandleError::UnsupportedWindowHandleType(h) => {
                write!(f, "Unsupported window handle type on Win32: {h:?}")
            }
        }
    }
}
