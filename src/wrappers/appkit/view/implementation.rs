use super::*;
use crate::wrappers::appkit::new_class_name;
use objc2::__framework_prelude::{AnyClass, AnyObject, Bool, Sel};
use objc2::ffi::objc_disposeClassPair;
use objc2::runtime::ClassBuilder;
use objc2::{msg_send, sel, ClassType};
use objc2_app_kit::{NSEvent, NSView};
use std::ffi::c_void;

/// # Safety
///
/// This class is going to be destroyed when its first instance gets deallocated.
///
/// The returned reference must NOT be used after that point.
pub unsafe fn create_view_class<V: ViewImpl>() -> &'static AnyClass {
    // Use unique class names so that there are no conflicts between different
    // instances. The class is deleted when the view is released. Previously,
    // the class was stored in a OnceCell after creation. This way, we didn't
    // have to recreate it each time a view was opened, but now we don't leave
    // any class definitions lying around when the plugin is closed.
    let class_name = new_class_name("BaseviewNSView_");

    let mut class = ClassBuilder::new(&class_name, NSView::class()).unwrap();

    // SAFETY: All of these function signatures are correct
    unsafe {
        class.add_method(
            sel!(acceptsFirstResponder),
            property_yes as extern "C-unwind" fn(_, _) -> _,
        );
        class.add_method(
            sel!(becomeFirstResponder),
            become_first_responder::<V> as extern "C-unwind" fn(_, _) -> _,
        );
        class.add_method(
            sel!(resignFirstResponder),
            resign_first_responder::<V> as extern "C-unwind" fn(_, _) -> _,
        );
        class.add_method(sel!(isFlipped), property_yes as extern "C-unwind" fn(_, _) -> _);
        class.add_method(
            sel!(preservesContentInLiveResize),
            property_no as extern "C-unwind" fn(_, _) -> _,
        );
        class.add_method(
            sel!(acceptsFirstMouse:),
            accepts_first_mouse as extern "C-unwind" fn(_, _, _) -> _,
        );

        class.add_method(
            sel!(windowShouldClose:),
            window_should_close::<V> as extern "C-unwind" fn(_, _, _) -> _,
        );
        class.add_method(sel!(dealloc), dealloc::<V> as extern "C-unwind" fn(_, _));
        class.add_method(
            sel!(viewWillMoveToWindow:),
            view_will_move_to_window::<V> as extern "C-unwind" fn(_, _, _) -> _,
        );
        class.add_method(sel!(hitTest:), hit_test::<V> as extern "C-unwind" fn(_, _, _) -> _);
        class.add_method(
            sel!(updateTrackingAreas:),
            update_tracking_areas::<V> as extern "C-unwind" fn(_, _, _) -> _,
        );

        class.add_method(sel!(mouseMoved:), mouse_moved::<V> as extern "C-unwind" fn(_, _, _) -> _);
        class.add_method(
            sel!(mouseDragged:),
            mouse_moved::<V> as extern "C-unwind" fn(_, _, _) -> _,
        );
        class.add_method(
            sel!(rightMouseDragged:),
            mouse_moved::<V> as extern "C-unwind" fn(_, _, _) -> _,
        );
        class.add_method(
            sel!(otherMouseDragged:),
            mouse_moved::<V> as extern "C-unwind" fn(_, _, _) -> _,
        );

        class.add_method(
            sel!(scrollWheel:),
            scroll_wheel::<V> as extern "C-unwind" fn(_, _, _) -> _,
        );

        class.add_method(
            sel!(viewDidChangeBackingProperties:),
            view_did_change_backing_properties::<V> as extern "C-unwind" fn(_, _, _) -> _,
        );

        class.add_method(
            sel!(draggingEntered:),
            dragging_entered::<V> as extern "C-unwind" fn(_, _, _) -> _,
        );
        class.add_method(
            sel!(prepareForDragOperation:),
            prepare_for_drag_operation::<V> as extern "C-unwind" fn(_, _, _) -> _,
        );
        class.add_method(
            sel!(performDragOperation:),
            perform_drag_operation::<V> as extern "C-unwind" fn(_, _, _) -> _,
        );
        class.add_method(
            sel!(draggingUpdated:),
            dragging_updated::<V> as extern "C-unwind" fn(_, _, _) -> _,
        );
        class.add_method(
            sel!(draggingExited:),
            dragging_exited::<V> as extern "C-unwind" fn(_, _, _) -> _,
        );
        class.add_method(
            sel!(handleNotification:),
            handle_notification::<V> as extern "C-unwind" fn(_, _, _) -> _,
        );

        class.add_method(sel!(mouseDown:), mouse_down::<V> as extern "C-unwind" fn(_, _, _));
        class.add_method(sel!(mouseUp:), mouse_up::<V> as extern "C-unwind" fn(_, _, _));
        class.add_method(
            sel!(rightMouseDown:),
            right_mouse_down::<V> as extern "C-unwind" fn(_, _, _),
        );
        class.add_method(sel!(rightMouseUp:), right_mouse_up::<V> as extern "C-unwind" fn(_, _, _));
        class.add_method(
            sel!(otherMouseDown:),
            other_mouse_down::<V> as extern "C-unwind" fn(_, _, _),
        );
        class.add_method(sel!(otherMouseUp:), other_mouse_up::<V> as extern "C-unwind" fn(_, _, _));

        class.add_method(sel!(mouseEntered:), mouse_entered::<V> as extern "C-unwind" fn(_, _, _));
        class.add_method(sel!(mouseExited:), mouse_exited::<V> as extern "C-unwind" fn(_, _, _));

        class.add_method(sel!(keyDown:), key_down::<V> as extern "C-unwind" fn(_, _, _));
        class.add_method(sel!(keyUp:), key_up::<V> as extern "C-unwind" fn(_, _, _));
        class.add_method(sel!(flagsChanged:), flags_changed::<V> as extern "C-unwind" fn(_, _, _));
    }

    class.add_ivar::<*mut c_void>(BASEVIEW_STATE_IVAR);

    class.register()
}

pub extern "C-unwind" fn dealloc<V: ViewImpl>(this: &mut AnyObject, _sel: Sel) {
    let class = this.class();
    View::<V>::free_inner(this, class);

    if let Some(superclass) = class.superclass() {
        let () = unsafe { msg_send![super(this, superclass), dealloc] };
    }

    // SAFETY: This is safe as long as nobody holds a reference to this class.
    // On the Baseview side, this is enforced by the safety contract in `create_view_class`
    unsafe { objc_disposeClassPair(class as *const _ as *mut _) }
}

extern "C-unwind" fn property_yes(_this: &NSView, _sel: Sel) -> Bool {
    Bool::YES
}

extern "C-unwind" fn property_no(_this: &NSView, _sel: Sel) -> Bool {
    Bool::NO
}

extern "C-unwind" fn accepts_first_mouse(_this: &NSView, _sel: Sel, _event: &NSEvent) -> Bool {
    Bool::YES
}

extern "C-unwind" fn become_first_responder<V: ViewImpl>(this: &View<V>, _sel: Sel) -> Bool {
    let Some(inner) = this.inner_ref() else { return false.into() };
    V::become_first_responder(inner).into()
}

extern "C-unwind" fn resign_first_responder<V: ViewImpl>(this: &View<V>, _sel: Sel) -> Bool {
    let Some(inner) = this.inner_ref() else { return true.into() };
    V::resign_first_responder(inner).into()
}

extern "C-unwind" fn window_should_close<V: ViewImpl>(
    this: &View<V>, _: Sel, _sender: &AnyObject,
) -> Bool {
    let Some(inner) = this.inner_ref() else { return true.into() };
    V::window_should_close(inner).into()
}

extern "C-unwind" fn view_did_change_backing_properties<V: ViewImpl>(
    this: &View<V>, _: Sel, _: &AnyObject,
) {
    let Some(inner) = this.inner_ref() else { return };
    V::view_did_change_backing_properties(inner);
}

extern "C-unwind" fn hit_test<V: ViewImpl>(
    this: &View<V>, _sel: Sel, point: NSPoint,
) -> Option<&NSView> {
    V::hit_test(this.inner_ref()?, point)
}

extern "C-unwind" fn view_will_move_to_window<V: ViewImpl>(
    this: &View<V>, _self: Sel, new_window: Option<&NSWindow>,
) {
    let Some(inner) = this.inner_ref() else { return };
    V::view_will_move_to_window(inner, new_window);
}

extern "C-unwind" fn update_tracking_areas<V: ViewImpl>(this: &View<V>, _self: Sel, _: &AnyObject) {
    let Some(inner) = this.inner_ref() else { return };
    V::update_tracking_areas(inner);
}

extern "C-unwind" fn mouse_moved<V: ViewImpl>(this: &View<V>, _sel: Sel, event: &NSEvent) {
    let Some(inner) = this.inner_ref() else { return };
    V::mouse_moved(inner, event);
}

extern "C-unwind" fn scroll_wheel<V: ViewImpl>(this: &View<V>, _: Sel, event: &NSEvent) {
    let Some(inner) = this.inner_ref() else { return };
    V::scroll_wheel(inner, event);
}

extern "C-unwind" fn dragging_entered<V: ViewImpl>(
    this: &View<V>, _sel: Sel, sender: Option<&ProtocolObject<dyn NSDraggingInfo>>,
) -> NSDragOperation {
    let Some(inner) = this.inner_ref() else { return NSDragOperation::None };
    V::dragging_entered(inner, sender)
}

extern "C-unwind" fn dragging_updated<V: ViewImpl>(
    this: &View<V>, _sel: Sel, sender: Option<&ProtocolObject<dyn NSDraggingInfo>>,
) -> NSDragOperation {
    let Some(inner) = this.inner_ref() else { return NSDragOperation::None };
    V::dragging_updated(inner, sender)
}

extern "C-unwind" fn prepare_for_drag_operation<V: ViewImpl>(
    this: &View<V>, _sel: Sel, sender: Option<&ProtocolObject<dyn NSDraggingInfo>>,
) -> Bool {
    let Some(inner) = this.inner_ref() else { return false.into() };
    V::prepare_for_drag_operation(inner, sender).into()
}

extern "C-unwind" fn perform_drag_operation<V: ViewImpl>(
    this: &View<V>, _sel: Sel, sender: Option<&ProtocolObject<dyn NSDraggingInfo>>,
) -> Bool {
    let Some(inner) = this.inner_ref() else { return false.into() };
    V::perform_drag_operation(inner, sender).into()
}

extern "C-unwind" fn dragging_exited<V: ViewImpl>(
    this: &View<V>, _sel: Sel, sender: Option<&ProtocolObject<dyn NSDraggingInfo>>,
) {
    let Some(inner) = this.inner_ref() else { return };
    V::dragging_exited(inner, sender)
}

extern "C-unwind" fn handle_notification<V: ViewImpl>(
    this: &View<V>, _cmd: Sel, notification: &NSNotification,
) {
    let Some(inner) = this.inner_ref() else { return };
    V::handle_notification(inner, notification)
}

extern "C-unwind" fn mouse_entered<V: ViewImpl>(this: &View<V>, _: Sel, _: &AnyObject) {
    let Some(inner) = this.inner_ref() else { return };
    V::mouse_entered(inner);
}

extern "C-unwind" fn mouse_exited<V: ViewImpl>(this: &View<V>, _: Sel, _: &AnyObject) {
    let Some(inner) = this.inner_ref() else { return };
    V::mouse_exited(inner);
}

extern "C-unwind" fn key_down<V: ViewImpl>(this: &View<V>, _: Sel, event: &NSEvent) {
    let Some(inner) = this.inner_ref() else { return };
    V::key_down(inner, event);
}

extern "C-unwind" fn key_up<V: ViewImpl>(this: &View<V>, _: Sel, event: &NSEvent) {
    let Some(inner) = this.inner_ref() else { return };
    V::key_up(inner, event);
}

extern "C-unwind" fn flags_changed<V: ViewImpl>(this: &View<V>, _: Sel, event: &NSEvent) {
    let Some(inner) = this.inner_ref() else { return };
    V::flags_changed(inner, event);
}

extern "C-unwind" fn mouse_down<V: ViewImpl>(this: &View<V>, _sel: Sel, event: &NSEvent) {
    let Some(inner) = this.inner_ref() else { return };
    V::mouse_down(inner, event);
}

extern "C-unwind" fn mouse_up<V: ViewImpl>(this: &View<V>, _sel: Sel, event: &NSEvent) {
    let Some(inner) = this.inner_ref() else { return };
    V::mouse_up(inner, event);
}

extern "C-unwind" fn right_mouse_down<V: ViewImpl>(this: &View<V>, _sel: Sel, event: &NSEvent) {
    let Some(inner) = this.inner_ref() else { return };
    V::right_mouse_down(inner, event);
}

extern "C-unwind" fn right_mouse_up<V: ViewImpl>(this: &View<V>, _sel: Sel, event: &NSEvent) {
    let Some(inner) = this.inner_ref() else { return };
    V::right_mouse_up(inner, event);
}

extern "C-unwind" fn other_mouse_down<V: ViewImpl>(this: &View<V>, _sel: Sel, event: &NSEvent) {
    let Some(inner) = this.inner_ref() else { return };
    V::other_mouse_down(inner, event);
}

extern "C-unwind" fn other_mouse_up<V: ViewImpl>(this: &View<V>, _sel: Sel, event: &NSEvent) {
    let Some(inner) = this.inner_ref() else { return };
    V::other_mouse_up(inner, event);
}
