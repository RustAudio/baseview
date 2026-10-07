use crate::platform;
use std::marker::PhantomData;

/// A weak handle to a [timer].
///
/// This handle can be used to identify which timer has been triggered during a [`on_timer`] event
/// using its [`PartialEq`] implementation.
///
/// It can also be cheaply [cloned](Clone) for convenience. All cloned handles represent the same
/// timer instance, and will always compare equal to one another.
///
/// Note this is a weak handle and is not tied at all to the actual timer's lifetime. A timer may
/// be invalidated and/or destroyed while this handle is still alive.
///
/// Conversely, dropping this handle has no effect on the lifetime of the timer, and it may still
/// fire after all handles have been dropped.
///
/// All timers are automatically destroyed and stopped when the associated window is destroyed.
///
/// [timer]: crate::WindowContext::create_timer
/// [`on_timer`]: crate::WindowHandler::on_timer
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
