#![deny(unsafe_code)]

use std::cell::Cell;
use std::error::Error;
use std::rc::Rc;
use std::sync::mpsc;
use std::thread;

use raw_window_handle::{HasRawWindowHandle, RawDisplayHandle, RawWindowHandle, XcbWindowHandle};

use crate::x11::event_loop::EventLoop;
use crate::x11::handle::{WindowHandle, WindowHandleReceiver};
use crate::{Event, MouseCursor, Size, WindowEvent, WindowHandler, WindowInfo, WindowOpenOptions};

use crate::x11::x11_window::X11Window;
use crate::x11::xcb_connection::XcbConnection;

pub(crate) struct Window {
    pub(crate) xcb_connection: XcbConnection,
    pub(crate) x11_window: X11Window,

    pub close_requested: Cell<bool>,

    mouse_cursor: Cell<MouseCursor>,
}

impl Window {
    pub fn open_parented<P, H, B>(parent: &P, options: WindowOpenOptions, build: B) -> WindowHandle
    where
        P: HasRawWindowHandle,
        H: WindowHandler,
        B: FnOnce(crate::Window) -> H,
        B: Send + 'static,
    {
        // Convert parent into something that X understands
        let parent_id = match parent.raw_window_handle() {
            RawWindowHandle::Xlib(h) => h.window as u32,
            RawWindowHandle::Xcb(h) => h.window,
            h => panic!("unsupported parent handle type {:?}", h),
        };

        let (tx, rx) = mpsc::sync_channel::<XcbWindowHandle>(1);

        let (parent_handle, window_handle) = WindowHandleReceiver::new();

        // TODO: handle window creation errors
        thread::spawn(move || {
            Self::window_thread(Some(parent_id), options, build, Some(tx), Some(parent_handle))
                .unwrap();
        });

        let raw_window_handle = rx.recv().unwrap();
        window_handle.window_opened(raw_window_handle)
    }

    pub fn open_blocking<H, B>(options: WindowOpenOptions, build: B)
    where
        H: WindowHandler,
        B: FnOnce(crate::Window) -> H,
        B: Send + 'static,
    {
        Self::window_thread(None, options, build, None, None).unwrap();
    }

    fn window_thread<H, B>(
        parent: Option<u32>, options: WindowOpenOptions, build: B,
        tx: Option<mpsc::SyncSender<XcbWindowHandle>>,
        handle_receiver: Option<WindowHandleReceiver>,
    ) -> Result<(), Box<dyn Error>>
    where
        H: WindowHandler,
        B: FnOnce(crate::Window) -> H,
        B: Send + 'static,
    {
        // Connect to the X server
        // FIXME: baseview error type instead of unwrap()
        let xcb_connection = XcbConnection::new().unwrap();

        let initial_size = options.size;
        let x11_window = X11Window::new(&xcb_connection, parent, options)?;

        let window_shared = Rc::new(Window {
            xcb_connection,
            x11_window,
            mouse_cursor: Cell::new(MouseCursor::default()),
            close_requested: Cell::new(false),
        });

        let mut handler = build(crate::Window::new(Rc::downgrade(&window_shared)));

        // Send an initial window resized event so the user is alerted of
        // the correct dpi scaling.
        let window_info =
            WindowInfo::from_logical_size(initial_size, window_shared.x11_window.dpi_scale_factor);
        handler.on_event(Event::Window(WindowEvent::Resized(window_info)));

        if let Some(tx) = tx {
            let _ = tx.send(window_shared.x11_window.raw_window_handle());
        }

        window_shared.x11_window.show(&window_shared.xcb_connection)?;

        EventLoop::new(window_shared, handler, handle_receiver).run()?;
        Ok(())
    }

    pub fn set_mouse_cursor(&self, mouse_cursor: MouseCursor) {
        if self.mouse_cursor.get() == mouse_cursor {
            return;
        }

        self.x11_window.set_mouse_cursor(&self.xcb_connection, mouse_cursor);

        self.mouse_cursor.set(mouse_cursor);
    }

    pub fn close(&self) {
        self.close_requested.set(true);
    }

    pub fn has_focus(&self) -> bool {
        unimplemented!()
    }

    pub fn focus(&self) {
        unimplemented!()
    }

    pub fn resize(&self, size: Size) {
        self.x11_window.resize(&self.xcb_connection, size)
    }

    #[cfg(feature = "opengl")]
    pub fn gl_context(&self) -> Option<std::rc::Weak<crate::gl::platform::GlContext>> {
        self.x11_window.gl_context()
    }

    pub fn raw_window_handle(&self) -> RawWindowHandle {
        self.x11_window.raw_window_handle().into()
    }

    pub fn raw_display_handle(&self) -> RawDisplayHandle {
        self.xcb_connection.raw_display_handle()
    }
}

pub fn copy_to_clipboard(_data: &str) {
    todo!()
}
