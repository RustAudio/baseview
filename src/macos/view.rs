use std::ffi::c_void;

use cocoa::appkit::{NSEvent, NSFilenamesPboardType, NSView, NSWindow};
use cocoa::base::{id, nil, BOOL, NO, YES};
use cocoa::foundation::{NSArray, NSPoint, NSRect, NSSize, NSUInteger};

use keyboard_types::Key;
use objc::{
    class,
    declare::ClassDecl,
    msg_send,
    runtime::{Class, Object, Protocol, Sel},
    sel, sel_impl, Encode, Encoding,
};
use uuid::Uuid;

use crate::MouseEvent::{ButtonPressed, ButtonReleased};
use crate::{
    DropData, DropEffect, Event, EventStatus, MouseButton, MouseEvent, Point, ScrollDelta, Size,
    WindowEvent, WindowInfo, WindowOpenOptions,
};

use super::keyboard::{from_nsstring, make_modifiers};
use super::window::WindowState;
use super::{
    NSDragOperationCopy, NSDragOperationGeneric, NSDragOperationLink, NSDragOperationMove,
    NSDragOperationNone,
};

/// Name of the field used to store the `WindowState` pointer.
pub(super) const BASEVIEW_STATE_IVAR: &str = "baseview_state";

#[link(name = "AppKit", kind = "framework")]
extern "C" {
    static NSWindowDidBecomeKeyNotification: id;
    static NSWindowDidResignKeyNotification: id;
}

macro_rules! add_simple_mouse_class_method {
    ($class:ident, $sel:ident, $event:expr) => {
        #[allow(non_snake_case)]
        extern "C" fn $sel(this: &Object, _: Sel, _: id){
            let state = unsafe { WindowState::from_view(this) };

            state.trigger_event(Event::Mouse($event));
        }

        $class.add_method(
            sel!($sel:),
            $sel as extern "C" fn(&Object, Sel, id),
        );
    };
}

/// Similar to [add_simple_mouse_class_method!], but this creates its own event object for the
/// press/release event and adds the active modifier keys to that event.
macro_rules! add_mouse_button_class_method {
    ($class:ident, $sel:ident, $event_ty:ident, $button:expr) => {
        #[allow(non_snake_case)]
        extern "C" fn $sel(this: &Object, _: Sel, event: id){
            let state = unsafe { WindowState::from_view(this) };

            let modifiers = unsafe { NSEvent::modifierFlags(event) };

            state.trigger_event(Event::Mouse($event_ty {
                button: $button,
                modifiers: make_modifiers(modifiers),
            }));
        }

        $class.add_method(
            sel!($sel:),
            $sel as extern "C" fn(&Object, Sel, id),
        );
    };
}

macro_rules! add_simple_keyboard_class_method {
    ($class:ident, $sel:ident) => {
        #[allow(non_snake_case)]
        extern "C" fn $sel(this: &Object, _: Sel, event: id){
            let state = unsafe { WindowState::from_view(this) };

            if let Some(key_event) = state.process_native_key_event(event){
                let status = state.trigger_event(Event::Keyboard(key_event));

                if let EventStatus::Ignored = status {
                    unsafe {
                        let superclass = msg_send![this, superclass];

                        let () = msg_send![super(this, superclass), $sel:event];
                    }
                }
            }
        }

        $class.add_method(
            sel!($sel:),
            $sel as extern "C" fn(&Object, Sel, id),
        );
    };
}

unsafe fn register_notification(observer: id, notification_name: id, object: id) {
    let notification_center: id = msg_send![class!(NSNotificationCenter), defaultCenter];

    let _: () = msg_send![
        notification_center,
        addObserver:observer
        selector:sel!(handleNotification:)
        name:notification_name
        object:object
    ];
}

pub(super) unsafe fn create_view(window_options: &WindowOpenOptions) -> id {
    let class = create_view_class();

    let view: id = msg_send![class, alloc];

    let size = window_options.size;

    view.initWithFrame_(NSRect::new(NSPoint::new(0., 0.), NSSize::new(size.width, size.height)));

    register_notification(view, NSWindowDidBecomeKeyNotification, nil);
    register_notification(view, NSWindowDidResignKeyNotification, nil);

    let _: id = msg_send![
        view,
        registerForDraggedTypes: NSArray::arrayWithObjects(nil, &[NSFilenamesPboardType])
    ];

    view
}

unsafe fn create_view_class() -> &'static Class {
    // Use unique class names so that there are no conflicts between different
    // instances. The class is deleted when the view is released. Previously,
    // the class was stored in a OnceCell after creation. This way, we didn't
    // have to recreate it each time a view was opened, but now we don't leave
    // any class definitions lying around when the plugin is closed.
    let class_name = format!("BaseviewNSView_{}", Uuid::new_v4().to_simple());
    let mut class = ClassDecl::new(&class_name, class!(NSView)).unwrap();

    class.add_method(
        sel!(acceptsFirstResponder),
        property_yes as extern "C" fn(&Object, Sel) -> BOOL,
    );
    class.add_method(
        sel!(becomeFirstResponder),
        become_first_responder as extern "C" fn(&Object, Sel) -> BOOL,
    );
    class.add_method(
        sel!(resignFirstResponder),
        resign_first_responder as extern "C" fn(&Object, Sel) -> BOOL,
    );
    class.add_method(sel!(isFlipped), property_yes as extern "C" fn(&Object, Sel) -> BOOL);
    class.add_method(
        sel!(preservesContentInLiveResize),
        property_no as extern "C" fn(&Object, Sel) -> BOOL,
    );
    class.add_method(
        sel!(acceptsFirstMouse:),
        accepts_first_mouse as extern "C" fn(&Object, Sel, id) -> BOOL,
    );

    class.add_method(
        sel!(windowShouldClose:),
        window_should_close as extern "C" fn(&Object, Sel, id) -> BOOL,
    );
    class.add_method(sel!(dealloc), dealloc as extern "C" fn(&mut Object, Sel));
    class.add_method(
        sel!(viewWillMoveToWindow:),
        view_will_move_to_window as extern "C" fn(&Object, Sel, id),
    );
    class.add_method(
        sel!(updateTrackingAreas:),
        update_tracking_areas as extern "C" fn(&Object, Sel, id),
    );

    class.add_method(sel!(mouseMoved:), mouse_moved as extern "C" fn(&Object, Sel, id));
    class.add_method(sel!(mouseDragged:), mouse_moved as extern "C" fn(&Object, Sel, id));
    class.add_method(sel!(rightMouseDragged:), mouse_moved as extern "C" fn(&Object, Sel, id));
    class.add_method(sel!(otherMouseDragged:), mouse_moved as extern "C" fn(&Object, Sel, id));

    class.add_method(sel!(scrollWheel:), scroll_wheel as extern "C" fn(&Object, Sel, id));

    class.add_method(
        sel!(viewDidChangeBackingProperties:),
        view_did_change_backing_properties as extern "C" fn(&Object, Sel, id),
    );

    class.add_method(
        sel!(draggingEntered:),
        dragging_entered as extern "C" fn(&Object, Sel, id) -> NSUInteger,
    );
    class.add_method(
        sel!(prepareForDragOperation:),
        prepare_for_drag_operation as extern "C" fn(&Object, Sel, id) -> BOOL,
    );
    class.add_method(
        sel!(performDragOperation:),
        perform_drag_operation as extern "C" fn(&Object, Sel, id) -> BOOL,
    );
    class.add_method(
        sel!(draggingUpdated:),
        dragging_updated as extern "C" fn(&Object, Sel, id) -> NSUInteger,
    );
    class.add_method(sel!(draggingExited:), dragging_exited as extern "C" fn(&Object, Sel, id));
    class.add_method(
        sel!(handleNotification:),
        handle_notification as extern "C" fn(&Object, Sel, id),
    );

    add_mouse_button_class_method!(class, mouseDown, ButtonPressed, MouseButton::Left);
    add_mouse_button_class_method!(class, mouseUp, ButtonReleased, MouseButton::Left);
    add_mouse_button_class_method!(class, rightMouseDown, ButtonPressed, MouseButton::Right);
    add_mouse_button_class_method!(class, rightMouseUp, ButtonReleased, MouseButton::Right);
    add_mouse_button_class_method!(class, otherMouseDown, ButtonPressed, MouseButton::Middle);
    add_mouse_button_class_method!(class, otherMouseUp, ButtonReleased, MouseButton::Middle);
    add_simple_mouse_class_method!(class, mouseEntered, MouseEvent::CursorEntered);
    add_simple_mouse_class_method!(class, mouseExited, MouseEvent::CursorLeft);

    // keyDown gets a custom impl that may route through NSTextInputContext
    // when a text-input view has focus (see `key_down` below). keyUp and
    // flagsChanged still go through the simple dispatch macro.
    class.add_method(sel!(keyDown:), key_down as extern "C" fn(&Object, Sel, id));

    // performKeyEquivalent: runs BEFORE keyDown: in AppKit's dispatch
    // order: AppKit offers the event to every view in the responder chain
    // (bottom-up), then to the window, then — only if no view claimed it —
    // falls through to keyDown:. Some plugin hosts (REAPER on macOS)
    // install key bindings at the window-performKeyEquivalent: level
    // (space = transport, etc). Without an override, our view returns NO
    // by default, the host's window-level handler claims the key, and
    // keyDown: on our view never fires — so users cannot type those
    // characters into a focused plugin text input. Tested repro in REAPER:
    // without this override, space cannot be typed into a focused textbox
    // and instead toggles REAPER's transport.
    //
    // Route through the same NSTextInputContext path as keyDown: and
    // claim the event when a text input has focus; return NO otherwise
    // so host accelerators work normally.
    class.add_method(
        sel!(performKeyEquivalent:),
        perform_key_equivalent as extern "C" fn(&Object, Sel, id) -> BOOL,
    );

    add_simple_keyboard_class_method!(class, keyUp);
    add_simple_keyboard_class_method!(class, flagsChanged);

    // NSTextInputClient protocol stubs.
    //
    // Cocoa text hosts (including REAPER on macOS) use protocol conformance
    // on the first-responder NSView as a signal that the view is a text
    // editor — if present, the host's key-binding pre-check (e.g. REAPER's
    // space-bar-is-transport) is bypassed and the key event is dispatched to
    // the view's NSTextInputContext instead. The methods below are mostly
    // inert sentinels; the real work still happens in the existing
    // `keyDown:` handler via `process_native_key_event` + `trigger_event`.
    if let Some(proto) = Protocol::get("NSTextInputClient") {
        class.add_protocol(proto);
    }

    class.add_method(
        sel!(hasMarkedText),
        has_marked_text as extern "C" fn(&Object, Sel) -> BOOL,
    );
    class.add_method(
        sel!(markedRange),
        marked_range as extern "C" fn(&Object, Sel) -> NSRange,
    );
    class.add_method(
        sel!(selectedRange),
        selected_range as extern "C" fn(&Object, Sel) -> NSRange,
    );
    class.add_method(
        sel!(setMarkedText:selectedRange:replacementRange:),
        set_marked_text as extern "C" fn(&Object, Sel, id, NSRange, NSRange),
    );
    class.add_method(sel!(unmarkText), unmark_text as extern "C" fn(&Object, Sel));
    class.add_method(
        sel!(validAttributesForMarkedText),
        valid_attributes_for_marked_text as extern "C" fn(&Object, Sel) -> id,
    );
    class.add_method(
        sel!(attributedSubstringForProposedRange:actualRange:),
        attributed_substring_for_proposed_range
            as extern "C" fn(&Object, Sel, NSRange, *mut c_void) -> id,
    );
    class.add_method(
        sel!(insertText:replacementRange:),
        insert_text as extern "C" fn(&Object, Sel, id, NSRange),
    );
    class.add_method(
        sel!(characterIndexForPoint:),
        character_index_for_point as extern "C" fn(&Object, Sel, NSPoint) -> NSUInteger,
    );
    class.add_method(
        sel!(firstRectForCharacterRange:actualRange:),
        first_rect_for_character_range as extern "C" fn(&Object, Sel, NSRange, *mut c_void) -> NSRect,
    );
    class.add_method(
        sel!(doCommandBySelector:),
        do_command_by_selector as extern "C" fn(&Object, Sel, Sel),
    );

    class.add_ivar::<*mut c_void>(BASEVIEW_STATE_IVAR);

    class.register()
}

const NSNOT_FOUND: NSUInteger = NSUInteger::MAX;

/// Local NSRange that implements `objc::Encode`. `cocoa::foundation::NSRange`
/// does not implement `Encode`, so we can't use it in `add_method` signatures
/// on the objc 0.2 API.
#[repr(C)]
#[derive(Copy, Clone)]
struct NSRange {
    location: NSUInteger,
    length: NSUInteger,
}

unsafe impl Encode for NSRange {
    fn encode() -> Encoding {
        let encoding = format!(
            "{{_NSRange={}{}}}",
            NSUInteger::encode().as_str(),
            NSUInteger::encode().as_str()
        );
        unsafe { Encoding::from_str(&encoding) }
    }
}

extern "C" fn has_marked_text(_this: &Object, _sel: Sel) -> BOOL {
    NO
}

extern "C" fn marked_range(_this: &Object, _sel: Sel) -> NSRange {
    NSRange { location: NSNOT_FOUND, length: 0 }
}

extern "C" fn selected_range(_this: &Object, _sel: Sel) -> NSRange {
    NSRange { location: NSNOT_FOUND, length: 0 }
}

extern "C" fn set_marked_text(
    _this: &Object,
    _sel: Sel,
    _text: id,
    _selected: NSRange,
    _replacement: NSRange,
) {
}

extern "C" fn unmark_text(_this: &Object, _sel: Sel) {}

extern "C" fn valid_attributes_for_marked_text(_this: &Object, _sel: Sel) -> id {
    unsafe { NSArray::arrayWithObjects(nil, &[]) }
}

extern "C" fn attributed_substring_for_proposed_range(
    _this: &Object,
    _sel: Sel,
    _range: NSRange,
    _actual_range: *mut c_void,
) -> id {
    nil
}

extern "C" fn insert_text(this: &Object, _sel: Sel, text: id, _replacement: NSRange) {
    // When `keyDown:` routes the event through NSTextInputContext, AppKit
    // runs the NSEvent through the keyboard layout, dead-key composition,
    // and any active IME, then calls back here with the finished string.
    // Use that decoded string for `Key::Character` — re-reading
    // `[event characters]` would miss IME-composed or dead-key-composed
    // characters because those aren't carried on any single NSEvent.
    //
    // `text` is documented as `id` — NSString in the common case,
    // NSAttributedString if the consumer set marked-text attributes.
    // Normalize to NSString via `-string` when needed.
    let text_str: id = unsafe {
        let is_attributed: BOOL = msg_send![text, isKindOfClass: class!(NSAttributedString)];
        if is_attributed == YES {
            msg_send![text, string]
        } else {
            text
        }
    };
    let decoded = from_nsstring(text_str);
    if decoded.is_empty() {
        return;
    }

    // Use the stored NSEvent for physical `Code` and `modifiers`; override
    // `Key::Character` with what AppKit handed us in `text`.
    let state = unsafe { WindowState::from_view(this) };
    let event = state.current_key_event();
    if event == nil {
        return;
    }
    if let Some(mut key_event) = state.process_native_key_event(event) {
        key_event.key = Key::Character(decoded);
        state.trigger_event(Event::Keyboard(key_event));
    }
}

extern "C" fn character_index_for_point(_this: &Object, _sel: Sel, _point: NSPoint) -> NSUInteger {
    NSNOT_FOUND
}

extern "C" fn first_rect_for_character_range(
    _this: &Object,
    _sel: Sel,
    _range: NSRange,
    _actual_range: *mut c_void,
) -> NSRect {
    NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(0.0, 0.0))
}

extern "C" fn do_command_by_selector(this: &Object, _sel: Sel, _selector: Sel) {
    // For non-printable commands (arrow keys, backspace, enter, escape,
    // etc.) AppKit calls this instead of `insertText:`. The stored NSEvent
    // already carries a mac-native keyCode that `process_native_key_event`
    // maps to the right `Code`, so we re-use the same dispatch path.
    let state = unsafe { WindowState::from_view(this) };
    let event = state.current_key_event();
    if event == nil {
        return;
    }
    if let Some(key_event) = state.process_native_key_event(event) {
        state.trigger_event(Event::Keyboard(key_event));
    }
}

/// Handler for `keyDown:`. When a text-input view has focus, route the
/// event through AppKit's NSTextInputContext so that:
///
/// - the host's key-binding pre-check (e.g. REAPER's space-bar transport
///   shortcut) is bypassed for text-input keys,
/// - IME input (Japanese, Chinese, Korean) and the macOS accent menu
///   work,
/// - dead-key composition (option+e then 'e' → 'é') works.
///
/// Otherwise fall back to the same dispatch as the simple keyboard
/// macro: translate the NSEvent into a `KeyboardEvent`, report it, and
/// forward to the superclass if the app didn't consume it.
extern "C" fn key_down(this: &Object, _sel: Sel, event: id) {
    let state = unsafe { WindowState::from_view(this) };

    // Route through the text-input pipeline only when the app reports a
    // text field has focus. Otherwise preserve the old behaviour so host
    // shortcuts still work (e.g. space toggles transport in REAPER when
    // no text field is focused).
    if state.has_text_focus() {
        state.set_current_key_event(event);
        let handled: BOOL = unsafe {
            let input_context: id = msg_send![this, inputContext];
            if input_context != nil {
                msg_send![input_context, handleEvent: event]
            } else {
                NO
            }
        };
        state.set_current_key_event(nil);

        if handled == YES {
            // NSTextInputContext dispatched via insertText: or
            // doCommandBySelector:, which already called trigger_event.
            // Do not call super — swallow the event so it does not bubble
            // up to the host window.
            return;
        }
        // Fall through. inputContext declined the event (e.g. a Cmd-modified
        // key), let the usual path handle it.
    }

    if let Some(key_event) = state.process_native_key_event(event) {
        let status = state.trigger_event(Event::Keyboard(key_event));

        if let EventStatus::Ignored = status {
            unsafe {
                let superclass = msg_send![this, superclass];
                let () = msg_send![super(this, superclass), keyDown: event];
            }
        }
    }
}

/// Handler for `performKeyEquivalent:`. See the comment on the
/// `add_method` registration above for why this override exists; in
/// short: hosts install window-level key bindings at this dispatch
/// stage, so without an override `keyDown:` never fires for keys the
/// host claims (e.g. space → REAPER transport), and users cannot type
/// them into a focused plugin text input.
///
/// When no text input is focused, return NO so AppKit continues to the
/// host's window-performKeyEquivalent: (host accelerators work
/// normally). When a text input is focused, route through
/// NSTextInputContext — same pipeline as `keyDown:` — and return
/// `handleEvent:`'s result so AppKit stops dispatch at our view when
/// the input context claimed the event.
extern "C" fn perform_key_equivalent(this: &Object, _sel: Sel, event: id) -> BOOL {
    let state = unsafe { WindowState::from_view(this) };

    if !state.has_text_focus() {
        return NO;
    }

    state.set_current_key_event(event);
    let handled: BOOL = unsafe {
        let input_context: id = msg_send![this, inputContext];
        if input_context != nil {
            msg_send![input_context, handleEvent: event]
        } else {
            NO
        }
    };
    state.set_current_key_event(nil);

    handled
}

extern "C" fn property_yes(_this: &Object, _sel: Sel) -> BOOL {
    YES
}

extern "C" fn property_no(_this: &Object, _sel: Sel) -> BOOL {
    NO
}

extern "C" fn accepts_first_mouse(_this: &Object, _sel: Sel, _event: id) -> BOOL {
    YES
}

extern "C" fn become_first_responder(this: &Object, _sel: Sel) -> BOOL {
    let state = unsafe { WindowState::from_view(this) };
    let is_key_window = unsafe {
        let window: id = msg_send![this, window];
        if window != nil {
            let is_key_window: BOOL = msg_send![window, isKeyWindow];
            is_key_window == YES
        } else {
            false
        }
    };
    if is_key_window {
        state.trigger_deferrable_event(Event::Window(WindowEvent::Focused));
    }

    // Mark our NSTextInputContext as the globally-active input context
    // while this view has focus. Required for REAPER's "Send all
    // keyboard input to plug-in" path: with that toggle enabled, REAPER
    // delivers modifier-combined keys (Cmd shortcuts, Cmd held alone)
    // to the active input context rather than routing them directly to
    // the NSView. Without `activate` here, those keys get dispatched
    // into an inactive context and are either dropped or misinterpreted
    // (Cmd held alone was observed firing `deleteBackward:` in practice).
    unsafe {
        let input_context: id = msg_send![this, inputContext];
        if input_context != nil {
            let _: () = msg_send![input_context, activate];
        }
    }

    YES
}

extern "C" fn resign_first_responder(this: &Object, _sel: Sel) -> BOOL {
    let state = unsafe { WindowState::from_view(this) };
    state.trigger_deferrable_event(Event::Window(WindowEvent::Unfocused));

    unsafe {
        let input_context: id = msg_send![this, inputContext];
        if input_context != nil {
            let _: () = msg_send![input_context, deactivate];
        }
    }

    YES
}

extern "C" fn window_should_close(this: &Object, _: Sel, _sender: id) -> BOOL {
    let state = unsafe { WindowState::from_view(this) };

    state.trigger_event(Event::Window(WindowEvent::WillClose));

    state.window_inner.close();

    NO
}

extern "C" fn dealloc(this: &mut Object, _sel: Sel) {
    unsafe {
        let class = msg_send![this, class];

        let superclass = msg_send![this, superclass];
        let () = msg_send![super(this, superclass), dealloc];

        // Delete class
        ::objc::runtime::objc_disposeClassPair(class);
    }
}

extern "C" fn view_did_change_backing_properties(this: &Object, _: Sel, _: id) {
    unsafe {
        let ns_window: *mut Object = msg_send![this, window];

        let scale_factor: f64 =
            if ns_window.is_null() { 1.0 } else { NSWindow::backingScaleFactor(ns_window) };

        let state = WindowState::from_view(this);

        let bounds: NSRect = msg_send![this, bounds];

        let new_window_info = WindowInfo::from_logical_size(
            Size::new(bounds.size.width, bounds.size.height),
            scale_factor,
        );

        let window_info = state.window_info.get();

        // Only send the event when the window's size has actually changed to be in line with the
        // other platform implementations
        if new_window_info.physical_size() != window_info.physical_size() {
            state.window_info.set(new_window_info);
            state.trigger_event(Event::Window(WindowEvent::Resized(new_window_info)));
        }
    }
}

/// Init/reinit tracking area
///
/// Info:
/// https://developer.apple.com/documentation/appkit/nstrackingarea
/// https://developer.apple.com/documentation/appkit/nstrackingarea/options
/// https://developer.apple.com/documentation/appkit/nstrackingareaoptions
unsafe fn reinit_tracking_area(this: &Object, tracking_area: *mut Object) {
    let options: usize = {
        let mouse_entered_and_exited = 0x01;
        let tracking_mouse_moved = 0x02;
        let tracking_cursor_update = 0x04;
        let tracking_active_in_active_app = 0x40;
        let tracking_in_visible_rect = 0x200;
        let tracking_enabled_during_mouse_drag = 0x400;

        mouse_entered_and_exited
            | tracking_mouse_moved
            | tracking_cursor_update
            | tracking_active_in_active_app
            | tracking_in_visible_rect
            | tracking_enabled_during_mouse_drag
    };

    let bounds: NSRect = msg_send![this, bounds];

    *tracking_area = msg_send![tracking_area,
        initWithRect:bounds
        options:options
        owner:this
        userInfo:nil
    ];
}

extern "C" fn view_will_move_to_window(this: &Object, _self: Sel, new_window: id) {
    unsafe {
        let tracking_areas: *mut Object = msg_send![this, trackingAreas];
        let tracking_area_count = NSArray::count(tracking_areas);

        if new_window == nil {
            if tracking_area_count != 0 {
                let tracking_area = NSArray::objectAtIndex(tracking_areas, 0);

                let _: () = msg_send![this, removeTrackingArea: tracking_area];
                let _: () = msg_send![tracking_area, release];
            }
        } else {
            if tracking_area_count == 0 {
                let class = Class::get("NSTrackingArea").unwrap();

                let tracking_area: *mut Object = msg_send![class, alloc];

                reinit_tracking_area(this, tracking_area);

                let _: () = msg_send![this, addTrackingArea: tracking_area];
            }

            let _: () = msg_send![new_window, setAcceptsMouseMovedEvents: YES];
            let _: () = msg_send![new_window, makeFirstResponder: this];
        }
    }

    unsafe {
        let superclass = msg_send![this, superclass];

        let () = msg_send![super(this, superclass), viewWillMoveToWindow: new_window];
    }
}

extern "C" fn update_tracking_areas(this: &Object, _self: Sel, _: id) {
    unsafe {
        let tracking_areas: *mut Object = msg_send![this, trackingAreas];
        let tracking_area = NSArray::objectAtIndex(tracking_areas, 0);

        reinit_tracking_area(this, tracking_area);
    }
}

extern "C" fn mouse_moved(this: &Object, _sel: Sel, event: id) {
    let state = unsafe { WindowState::from_view(this) };

    let point: NSPoint = unsafe {
        let point = NSEvent::locationInWindow(event);

        msg_send![this, convertPoint:point fromView:nil]
    };
    let modifiers = unsafe { NSEvent::modifierFlags(event) };

    let position = Point { x: point.x, y: point.y };

    state.trigger_event(Event::Mouse(MouseEvent::CursorMoved {
        position,
        modifiers: make_modifiers(modifiers),
    }));
}

extern "C" fn scroll_wheel(this: &Object, _: Sel, event: id) {
    let state = unsafe { WindowState::from_view(this) };

    let delta = unsafe {
        let x = NSEvent::scrollingDeltaX(event) as f32;
        let y = NSEvent::scrollingDeltaY(event) as f32;

        if NSEvent::hasPreciseScrollingDeltas(event) != NO {
            ScrollDelta::Pixels { x, y }
        } else {
            ScrollDelta::Lines { x, y }
        }
    };

    let modifiers = unsafe { NSEvent::modifierFlags(event) };

    state.trigger_event(Event::Mouse(MouseEvent::WheelScrolled {
        delta,
        modifiers: make_modifiers(modifiers),
    }));
}

fn get_drag_position(sender: id) -> Point {
    let point: NSPoint = unsafe { msg_send![sender, draggingLocation] };
    Point::new(point.x, point.y)
}

fn get_drop_data(sender: id) -> DropData {
    if sender == nil {
        return DropData::None;
    }

    unsafe {
        let pasteboard: id = msg_send![sender, draggingPasteboard];
        let file_list: id = msg_send![pasteboard, propertyListForType: NSFilenamesPboardType];

        if file_list == nil {
            return DropData::None;
        }

        let mut files = vec![];
        for i in 0..NSArray::count(file_list) {
            let data = NSArray::objectAtIndex(file_list, i);
            files.push(from_nsstring(data).into());
        }

        DropData::Files(files)
    }
}

fn on_event(window_state: &WindowState, event: MouseEvent) -> NSUInteger {
    let event_status = window_state.trigger_event(Event::Mouse(event));
    match event_status {
        EventStatus::AcceptDrop(DropEffect::Copy) => NSDragOperationCopy,
        EventStatus::AcceptDrop(DropEffect::Move) => NSDragOperationMove,
        EventStatus::AcceptDrop(DropEffect::Link) => NSDragOperationLink,
        EventStatus::AcceptDrop(DropEffect::Scroll) => NSDragOperationGeneric,
        _ => NSDragOperationNone,
    }
}

extern "C" fn dragging_entered(this: &Object, _sel: Sel, sender: id) -> NSUInteger {
    let state = unsafe { WindowState::from_view(this) };
    let modifiers = state.keyboard_state().last_mods();
    let drop_data = get_drop_data(sender);

    let event = MouseEvent::DragEntered {
        position: get_drag_position(sender),
        modifiers: make_modifiers(modifiers),
        data: drop_data,
    };

    on_event(&state, event)
}

extern "C" fn dragging_updated(this: &Object, _sel: Sel, sender: id) -> NSUInteger {
    let state = unsafe { WindowState::from_view(this) };
    let modifiers = state.keyboard_state().last_mods();
    let drop_data = get_drop_data(sender);

    let event = MouseEvent::DragMoved {
        position: get_drag_position(sender),
        modifiers: make_modifiers(modifiers),
        data: drop_data,
    };

    on_event(&state, event)
}

extern "C" fn prepare_for_drag_operation(_this: &Object, _sel: Sel, _sender: id) -> BOOL {
    // Always accept drag operation if we get this far
    // This function won't be called unless dragging_entered/updated
    // has returned an acceptable operation
    YES
}

extern "C" fn perform_drag_operation(this: &Object, _sel: Sel, sender: id) -> BOOL {
    let state = unsafe { WindowState::from_view(this) };
    let modifiers = state.keyboard_state().last_mods();
    let drop_data = get_drop_data(sender);

    let event = MouseEvent::DragDropped {
        position: get_drag_position(sender),
        modifiers: make_modifiers(modifiers),
        data: drop_data,
    };

    let event_status = state.trigger_event(Event::Mouse(event));
    match event_status {
        EventStatus::AcceptDrop(_) => YES,
        _ => NO,
    }
}

extern "C" fn dragging_exited(this: &Object, _sel: Sel, _sender: id) {
    let state = unsafe { WindowState::from_view(this) };

    on_event(&state, MouseEvent::DragLeft);
}

extern "C" fn handle_notification(this: &Object, _cmd: Sel, notification: id) {
    unsafe {
        let state = WindowState::from_view(this);

        // The subject of the notication, in this case an NSWindow object.
        let notification_object: id = msg_send![notification, object];

        // The NSWindow object associated with our NSView.
        let window: id = msg_send![this, window];

        let first_responder: id = msg_send![window, firstResponder];

        // Only trigger focus events if the NSWindow that's being notified about is our window,
        // and if the window's first responder is our NSView.
        // If the first responder isn't our NSView, the focus events will instead be triggered
        // by the becomeFirstResponder and resignFirstResponder methods on the NSView itself.
        if notification_object == window && first_responder == this as *const Object as id {
            let is_key_window: BOOL = msg_send![window, isKeyWindow];
            state.trigger_event(Event::Window(if is_key_window == YES {
                WindowEvent::Focused
            } else {
                WindowEvent::Unfocused
            }));
        }
    }
}
