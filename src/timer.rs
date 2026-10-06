use crate::platform;
use std::marker::PhantomData;

#[derive(PartialEq, Eq, Clone)]
#[repr(transparent)]
pub struct TimerHandle {
    inner: platform::TimerHandle,
    _nonsend: PhantomData<*mut ()>, // Ensures this is !Send & !Sync on all platforms
}

impl TimerHandle {
    pub(crate) fn from_ref(handle: &platform::TimerHandle) -> &TimerHandle {
        // SAFETY: This is repr(transparent)
        unsafe { core::mem::transmute::<&platform::TimerHandle, &TimerHandle>(handle) }
    }
}

impl From<platform::TimerHandle> for TimerHandle {
    #[inline]
    fn from(value: platform::TimerHandle) -> Self {
        Self { inner: value, _nonsend: PhantomData }
    }
}

impl<'a> From<&'a platform::TimerHandle> for &'a TimerHandle {
    #[inline]
    fn from(value: &'a platform::TimerHandle) -> Self {
        TimerHandle::from_ref(value)
    }
}

impl PartialEq<&TimerHandle> for TimerHandle {
    #[inline]
    fn eq(&self, other: &&TimerHandle) -> bool {
        self == *other
    }
}

impl PartialEq<TimerHandle> for &TimerHandle {
    #[inline]
    fn eq(&self, other: &TimerHandle) -> bool {
        *self == other
    }
}
