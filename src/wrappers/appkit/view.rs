use crate::WindowOpenOptions;
use objc2::__framework_prelude::{Allocated, AnyClass, ProtocolObject, Retained};
use objc2::runtime::AnyObject;
use objc2::{msg_send, sel, Encoding, Message, RefEncode};
use objc2_app_kit::{
    NSDragOperation, NSDraggingInfo, NSEvent, NSFilenamesPboardType, NSView, NSWindow,
    NSWindowDidBecomeKeyNotification, NSWindowDidResignKeyNotification,
};
use objc2_core_foundation::{CGRect, CFUUID};
use objc2_foundation::{NSArray, NSNotification, NSNotificationCenter, NSPoint, NSRect, NSSize};
use raw_window_handle::{AppKitWindowHandle, HasRawWindowHandle, RawWindowHandle};
use std::ffi::{c_void, CStr, CString};
use std::marker::PhantomData;
use std::ops::Deref;

mod implementation;

/// Name of the field used to store the `WindowState` pointer.
const BASEVIEW_STATE_IVAR: &CStr = c"baseview_state";

#[repr(C)]
pub struct View<V> {
    parent: NSView,
    _inner: PhantomData<ViewInner<V>>,
}

// SAFETY: TODO
unsafe impl<V> RefEncode for View<V> {
    const ENCODING_REF: Encoding = NSView::ENCODING_REF;
}

// SAFETY: TODO
unsafe impl<V> Message for View<V> {}

impl<V> Deref for View<V> {
    type Target = NSView;

    fn deref(&self) -> &Self::Target {
        &self.parent
    }
}

impl<V: ViewImpl> View<V> {
    pub fn new(frame: CGRect, inner: V, init: impl FnOnce(ViewRef<V>)) -> Retained<View<V>> {
        // SAFETY: We don't access this reference after this function
        let class = unsafe { implementation::create_view_class::<V>() };

        // SAFETY: This function is valid to call, and Allocated<View> is the correct type for the
        // returned pointer
        let view: Allocated<View<V>> = unsafe { msg_send![class, alloc] };
        Self::set_inner(&view, class, ViewInner { inner });

        let view: Retained<View<V>> = unsafe { msg_send![view, initWithFrame: frame] };

        init(view.inner_ref());

        view
    }

    fn set_inner(view: &Allocated<View<V>>, class: &AnyClass, inner: ViewInner<V>) {
        let inner = Box::new(inner);
        let ivar = class.instance_variable(BASEVIEW_STATE_IVAR).unwrap();
        let ivar_target = unsafe { &*Allocated::as_ptr(&view).cast() };
        let ivar = unsafe { ivar.load_ptr::<*mut c_void>(ivar_target) };
        unsafe { ivar.write(Box::into_raw(inner).cast()) };
    }

    fn free_inner(this: &AnyObject, class: &AnyClass) {
        let ivar = class.instance_variable(BASEVIEW_STATE_IVAR).unwrap();
        let ivar = unsafe { ivar.load_ptr::<*mut c_void>(this) };
        let raw = unsafe { ivar.read() };
        let inner = unsafe { Box::<ViewInner<V>>::from_raw(raw.cast()) };
        unsafe { ivar.write(core::ptr::null_mut()) };
        drop(inner);
    }

    fn get_inner(&self) -> &ViewInner<V> {
        let ivar = self.class().instance_variable(BASEVIEW_STATE_IVAR).unwrap();
        let ivar = unsafe { ivar.load::<*mut c_void>(self) };
        unsafe { ivar.cast::<ViewInner<V>>().as_ref() }.unwrap()
    }

    pub fn inner(&self) -> &V {
        &self.get_inner().inner
    }

    pub fn inner_ref(&self) -> ViewRef<V> {
        ViewRef { view: self, inner: self.inner() }
    }
}

pub struct ViewInner<V> {
    inner: V,
}

fn new_class_name() -> CString {
    // PANIC: CFUUIDCreate is not documented to return NULL.
    let uuid = CFUUID::new(None).unwrap();
    // PANIC: CFUUIDCreateString is not documented to return NULL.
    let uuid_str = CFUUID::new_string(None, Some(&uuid)).unwrap();

    let class_name = format!("BaseviewNSView_{uuid_str}");
    // PANIC: This cannot have any NULL bytes
    CString::new(class_name).unwrap()
}

pub struct ViewRef<'a, V> {
    pub view: &'a View<V>,
    pub inner: &'a V,
}

impl<'a, V> Clone for ViewRef<'a, V> {
    fn clone(&self) -> Self {
        Self { view: self.view, inner: self.inner }
    }
}

impl<'a, V> Copy for ViewRef<'a, V> {}

impl<V> Deref for ViewRef<'_, V> {
    type Target = V;

    fn deref(&self) -> &Self::Target {
        self.inner
    }
}

pub trait ViewImpl: Sized {
    fn init(&self, view: &Retained<View<Self>>);
    fn become_first_responder(this: ViewRef<Self>) -> bool;
    fn resign_first_responder(this: ViewRef<Self>) -> bool;
    fn window_should_close(this: ViewRef<Self>) -> bool;
    fn view_did_change_backing_properties(this: ViewRef<Self>);
    fn hit_test(this: ViewRef<Self>, point: NSPoint) -> Option<&NSView>;
    fn view_will_move_to_window(this: ViewRef<Self>, new_window: Option<&NSWindow>);
    fn update_tracking_areas(this: ViewRef<Self>);
    fn mouse_moved(this: ViewRef<Self>, event: &NSEvent);
    fn scroll_wheel(this: ViewRef<Self>, event: &NSEvent);
    fn dragging_entered(
        this: ViewRef<Self>, sender: Option<&ProtocolObject<dyn NSDraggingInfo>>,
    ) -> NSDragOperation;
    fn dragging_updated(
        this: ViewRef<Self>, sender: Option<&ProtocolObject<dyn NSDraggingInfo>>,
    ) -> NSDragOperation;
    fn prepare_for_drag_operation(
        this: ViewRef<Self>, sender: Option<&ProtocolObject<dyn NSDraggingInfo>>,
    ) -> bool;
    fn perform_drag_operation(
        this: ViewRef<Self>, sender: Option<&ProtocolObject<dyn NSDraggingInfo>>,
    ) -> bool;
    fn dragging_exited(this: ViewRef<Self>, sender: Option<&ProtocolObject<dyn NSDraggingInfo>>);
    fn handle_notification(this: ViewRef<Self>, notification: &NSNotification);

    fn mouse_down(this: ViewRef<Self>, event: &NSEvent);
    fn mouse_up(this: ViewRef<Self>, event: &NSEvent);
    fn right_mouse_down(this: ViewRef<Self>, event: &NSEvent);
    fn right_mouse_up(this: ViewRef<Self>, event: &NSEvent);
    fn other_mouse_down(this: ViewRef<Self>, event: &NSEvent);
    fn other_mouse_up(this: ViewRef<Self>, event: &NSEvent);

    fn mouse_entered(this: ViewRef<Self>);
    fn mouse_exited(this: ViewRef<Self>);

    fn key_down(this: ViewRef<Self>, event: &NSEvent);
    fn key_up(this: ViewRef<Self>, event: &NSEvent);
    fn flags_changed(this: ViewRef<Self>, event: &NSEvent);
}
