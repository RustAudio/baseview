use block2::RcBlock;
use objc2::rc::{Retained, Weak};
use objc2::runtime::{NSObjectProtocol, ProtocolObject};
use objc2_app_kit::{NSWindowDidBecomeKeyNotification, NSWindowDidResignKeyNotification};
use objc2_foundation::{NSNotification, NSNotificationCenter};
use std::ptr::NonNull;

pub struct NotificationCenterObserver {
    notification_center: Weak<NSNotificationCenter>,
    handlers: [Retained<ProtocolObject<dyn NSObjectProtocol>>; 2],
}

impl NotificationCenterObserver {
    pub fn register_window_key_change(handler: impl Fn(&NSNotification) + 'static) -> Self {
        let notification_center = NSNotificationCenter::defaultCenter();

        let block = RcBlock::new(move |n: NonNull<NSNotification>| {
            let n = unsafe { n.as_ref() };
            handler(n);
        });

        // SAFETY: block does not need to be sendable, as a `None` queue specifies the block
        // will run on the calling thread, which for Window operations is the main thread.
        let handlers = unsafe {
            [
                notification_center.addObserverForName_object_queue_usingBlock(
                    Some(NSWindowDidBecomeKeyNotification),
                    None,
                    None,
                    &block,
                ),
                notification_center.addObserverForName_object_queue_usingBlock(
                    Some(NSWindowDidResignKeyNotification),
                    None,
                    None,
                    &block,
                ),
            ]
        };

        Self { notification_center: Weak::from_retained(&notification_center), handlers }
    }
}

impl Drop for NotificationCenterObserver {
    fn drop(&mut self) {
        // If the notification center is already gone, then no need to unregister, the handler is already released
        let Some(notification_center) = self.notification_center.load() else { return };

        for h in &self.handlers {
            unsafe { notification_center.removeObserver(h.as_ref()) };
        }
    }
}
