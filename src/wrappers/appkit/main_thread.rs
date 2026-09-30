use dispatch2::DispatchQueue;
use objc2::rc::Weak;
use objc2::{MainThreadMarker, Message};
use std::mem::ManuallyDrop;
use std::time::Duration;

// This is like MainThreadBound<Weak<T>>, but uses async dispatch
pub struct MainThreadBoundWeak<T: Message + 'static>(ManuallyDrop<Weak<T>>);

// SAFETY: The inner value is guaranteed to originate from the main thread
// because T is Message.
//
// Finally, the value is dropped on the main thread in `Drop`.
unsafe impl<T: Message + 'static> Send for MainThreadBoundWeak<T> {}

// SAFETY: We do not provide access to the inner value.
unsafe impl<T: Message + 'static> Sync for MainThreadBoundWeak<T> {}

impl<T: Message + 'static> Drop for MainThreadBoundWeak<T> {
    #[inline]
    fn drop(&mut self) {
        if MainThreadMarker::new().is_some() {
            // SAFETY: The value is dropped on the main thread, which is
            // the same thread that it originated from (guaranteed by
            // `new` taking `MainThreadMarker`).
            //
            // Additionally, the value is never used again after this
            // point.
            unsafe { ManuallyDrop::drop(&mut self.0) };
        } else {
            let weak = MTWrapper(unsafe { ManuallyDrop::take(&mut self.0) });

            DispatchQueue::main().exec_async(move || {
                drop(weak);
            });
        }
    }
}

impl<T: Message + 'static> Clone for MainThreadBoundWeak<T> {
    fn clone(&self) -> Self {
        // This is actually safe to do off of the main thread
        Self(ManuallyDrop::new(Weak::clone(&self.0)))
    }
}

impl<T: Message + 'static> MainThreadBoundWeak<T> {
    pub fn new(inner: Weak<T>) -> Self {
        Self(ManuallyDrop::new(inner))
    }

    pub fn use_on_main_thread_after(
        &self, duration: Duration, handler: impl FnOnce(Weak<T>) + Send + 'static,
    ) {
        if duration.is_zero() {
            self.use_on_main_thread(handler);
            return;
        }

        let Ok(time) = duration.try_into() else {
            return;
        };

        let handler = {
            let handle = MTWrapper(Weak::clone(&self.0));
            let handler = MTWrapper::new_handler(handler);
            move || handler(handle)
        };

        let _ = DispatchQueue::main().after(time, handler);
    }

    pub fn use_on_main_thread(&self, handler: impl FnOnce(Weak<T>) + Send + 'static) {
        if MainThreadMarker::new().is_some() {
            handler(Weak::clone(&self.0));
            return;
        }

        let handler = {
            let handle = MTWrapper(Weak::clone(&self.0));
            let handler = MTWrapper::new_handler(handler);
            move || handler(handle)
        };

        DispatchQueue::main().exec_async(handler);
    }
}

struct MTWrapper<T>(Weak<T>);

impl<T> MTWrapper<T> {
    pub fn new_handler(
        handler: impl FnOnce(Weak<T>) + Send + 'static,
    ) -> impl FnOnce(MTWrapper<T>) + Send + 'static {
        |h| handler(h.0)
    }
}

unsafe impl<T> Send for MTWrapper<T> {}
unsafe impl<T> Sync for MTWrapper<T> {}
