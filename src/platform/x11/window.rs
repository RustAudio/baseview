use std::cell::Cell;
use std::error::Error;
use std::num::NonZero;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::Arc;
use std::thread::{self, JoinHandle};

use raw_window_handle::{HasWindowHandle, RawWindowHandle};

use x11rb::connection::Connection;
use x11rb::protocol::xproto::{
    AtomEnum, ConnectionExt, CreateGCAux, CreateWindowAux, EventMask, PropMode, WindowClass,
};
use x11rb::wrapper::ConnectionExt as _;

use super::X11Connection;
use super::{event_loop::EventLoop, visual_info::WindowVisualConfig};
use crate::context::WindowContext;
use crate::platform::x11::window_shared::WindowInner;
use crate::{Event, WindowEvent, WindowHandler, WindowInfo, WindowOpenOptions, WindowScalePolicy};

pub struct WindowHandle {
    window_id: Option<NonZero<x11rb::protocol::xproto::Window>>,
    event_loop_handle: Cell<Option<JoinHandle<()>>>,
    close_requested: Arc<AtomicBool>,
    is_open: Arc<AtomicBool>,
}

impl WindowHandle {
    pub fn close(&self) {
        self.close_requested.store(true, Ordering::Relaxed);
        if let Some(event_loop) = self.event_loop_handle.take() {
            let _ = event_loop.join();
        }
    }

    pub fn is_open(&self) -> bool {
        self.is_open.load(Ordering::Relaxed)
    }
}

pub(crate) struct ParentHandle {
    close_requested: Arc<AtomicBool>,
    is_open: Arc<AtomicBool>,
}

impl ParentHandle {
    pub fn new() -> (Self, WindowHandle) {
        let close_requested = Arc::new(AtomicBool::new(false));
        let is_open = Arc::new(AtomicBool::new(true));
        let handle = WindowHandle {
            window_id: None,
            event_loop_handle: None.into(),
            close_requested: Arc::clone(&close_requested),
            is_open: Arc::clone(&is_open),
        };

        (Self { close_requested, is_open }, handle)
    }

    pub fn parent_did_drop(&self) -> bool {
        self.close_requested.load(Ordering::Relaxed)
    }
}

impl Drop for ParentHandle {
    fn drop(&mut self) {
        self.is_open.store(false, Ordering::Relaxed);
    }
}

pub struct Window;

type WindowOpenResult = Result<NonZero<x11rb::protocol::xproto::Window>, ()>;

impl Window {
    pub fn open_parented<H: WindowHandler>(
        parent: &impl HasWindowHandle, options: WindowOpenOptions,
        build: impl FnOnce(WindowContext) -> H + Send + 'static,
    ) -> WindowHandle {
        // Convert parent into something that X understands
        let parent_id = match parent.window_handle().unwrap().as_raw() {
            RawWindowHandle::Xlib(h) => h.window as u32,
            RawWindowHandle::Xcb(h) => h.window.get(),
            h => panic!("unsupported parent handle type {:?}", h),
        };

        let (tx, rx) = mpsc::sync_channel::<WindowOpenResult>(1);
        let (parent_handle, mut window_handle) = ParentHandle::new();
        let join_handle = thread::spawn(move || {
            Self::window_thread(Some(parent_id), options, build, tx.clone(), Some(parent_handle))
                .unwrap();
        });

        let raw_window_handle = rx.recv().unwrap().unwrap();
        window_handle.window_id = Some(raw_window_handle);
        window_handle.event_loop_handle = Some(join_handle).into();
        window_handle
    }

    pub fn open_blocking<H: WindowHandler>(
        options: WindowOpenOptions, build: impl FnOnce(WindowContext) -> H + Send + 'static,
    ) {
        let (tx, rx) = mpsc::sync_channel::<WindowOpenResult>(1);

        let thread = thread::spawn(move || {
            Self::window_thread(None, options, build, tx, None).unwrap();
        });

        let _ = rx.recv().unwrap().unwrap();

        thread.join().unwrap_or_else(|err| {
            eprintln!("Window thread panicked: {:#?}", err);
        });
    }

    fn window_thread<H: WindowHandler>(
        parent: Option<u32>, options: WindowOpenOptions,
        build: impl FnOnce(WindowContext) -> H + Send + 'static,
        tx: mpsc::SyncSender<WindowOpenResult>, parent_handle: Option<ParentHandle>,
    ) -> Result<(), Box<dyn Error>> {
        // Connect to the X server
        // FIXME: baseview error type instead of unwrap()
        let xcb_connection = X11Connection::new()?;

        // Setup xkbcommon
        let xkb_state = crate::wrappers::xkbcommon::XkbcommonState::new(&xcb_connection);

        // Get screen information
        let screen = xcb_connection.screen();
        let parent_id = parent.unwrap_or(screen.root);

        let gc_id = xcb_connection.conn.generate_id()?;
        xcb_connection.conn.create_gc(
            gc_id,
            parent_id,
            &CreateGCAux::new().foreground(screen.black_pixel).graphics_exposures(0),
        )?;

        let scaling = match options.scale {
            WindowScalePolicy::SystemScaleFactor => xcb_connection.get_scaling().unwrap_or(1.0),
            WindowScalePolicy::ScaleFactor(scale) => scale,
        };

        let window_info = WindowInfo::from_logical_size(options.size, scaling);

        #[cfg(feature = "opengl")]
        let visual_info =
            WindowVisualConfig::find_best_visual_config_for_gl(&xcb_connection, options.gl_config)?;

        #[cfg(not(feature = "opengl"))]
        let visual_info = WindowVisualConfig::find_best_visual_config(&xcb_connection)?;

        let Some(window_id) = NonZero::new(xcb_connection.conn.generate_id()?) else {
            unreachable!();
        };

        xcb_connection.conn.create_window(
            visual_info.visual_depth,
            window_id.get(),
            parent_id,
            0,                                         // x coordinate of the new window
            0,                                         // y coordinate of the new window
            window_info.physical_size().width as u16,  // window width
            window_info.physical_size().height as u16, // window height
            0,                                         // window border
            WindowClass::INPUT_OUTPUT,
            visual_info.visual_id,
            &CreateWindowAux::new()
                .event_mask(
                    EventMask::EXPOSURE
                        | EventMask::POINTER_MOTION
                        | EventMask::BUTTON_PRESS
                        | EventMask::BUTTON_RELEASE
                        | EventMask::KEY_PRESS
                        | EventMask::KEY_RELEASE
                        | EventMask::STRUCTURE_NOTIFY
                        | EventMask::ENTER_WINDOW
                        | EventMask::LEAVE_WINDOW
                        | EventMask::FOCUS_CHANGE,
                )
                // As mentioned above, these two values are needed to be able to create a window
                // with a depth of 32-bits when the parent window has a different depth
                .colormap(visual_info.color_map)
                .border_pixel(0),
        )?;
        xcb_connection.conn.map_window(window_id.get())?;

        // Change window title
        let title = options.title;
        xcb_connection.conn.change_property8(
            PropMode::REPLACE,
            window_id.get(),
            AtomEnum::WM_NAME,
            AtomEnum::STRING,
            title.as_bytes(),
        )?;

        xcb_connection.conn.change_property32(
            PropMode::REPLACE,
            window_id.get(),
            xcb_connection.atoms.WM_PROTOCOLS,
            AtomEnum::ATOM,
            &[xcb_connection.atoms.WM_DELETE_WINDOW],
        )?;

        // Enable drag and drop (TODO: Make this toggleable?)
        xcb_connection.conn.change_property32(
            PropMode::REPLACE,
            window_id.get(),
            xcb_connection.atoms.XdndAware,
            AtomEnum::ATOM,
            &[5u32], // Latest version; hasn't changed since 2002
        )?;

        xcb_connection.conn.flush()?;
        let xcb_connection = Rc::new(xcb_connection);

        // TODO: These APIs could use a couple tweaks now that everything is internal and there is
        //       no error handling anymore at this point. Everything is more or less unchanged
        //       compared to when raw-gl-context was a separate crate.
        #[cfg(feature = "opengl")]
        let gl_context = visual_info.fb_config.map(|fb_config| {
            use std::ffi::c_ulong;

            let window = window_id.get() as c_ulong;

            // Because of the visual negotation we had to take some extra steps to create this context
            let context =
                super::gl::GlContextInner::create(window, Rc::clone(&xcb_connection), fb_config)
                    .expect("Could not create OpenGL context");

            Rc::new(context)
        });

        let inner = Rc::new(WindowInner::new(
            xcb_connection,
            window_id,
            window_info,
            #[cfg(feature = "opengl")]
            gl_context,
        ));

        let handler = build(WindowContext::new(Rc::clone(&inner)));

        // Send an initial window resized event so the user is alerted of
        // the correct dpi scaling.
        handler.on_event(Event::Window(WindowEvent::Resized(window_info)));

        let _ = tx.send(Ok(window_id));

        EventLoop::new(inner, handler, parent_handle, xkb_state).run()?;

        Ok(())
    }
}

pub fn copy_to_clipboard(_data: &str) {
    todo!()
}
