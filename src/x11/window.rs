use std::error::Error;
use std::ffi::c_void;
use std::os::fd::AsRawFd;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::Arc;
use std::thread;
use std::time::*;

use raw_window_handle::{
    HasRawDisplayHandle, HasRawWindowHandle, RawDisplayHandle, RawWindowHandle, XlibDisplayHandle,
    XlibWindowHandle,
};

use x11rb::connection::Connection;
use x11rb::protocol::xproto::{
    AtomEnum, ChangeWindowAttributesAux, ColormapAlloc, ConfigureWindowAux, ConnectionExt as _,
    CreateGCAux, CreateWindowAux, EventMask, PropMode, Screen, VisualClass, Visualid,
    Window as XWindow, WindowClass,
};
use x11rb::protocol::Event as XEvent;
use x11rb::wrapper::ConnectionExt as _;

use super::XcbConnection;
use crate::{
    Event, MouseButton, MouseCursor, MouseEvent, PhyPoint, PhySize, ScrollDelta, Size, WindowEvent,
    WindowHandler, WindowInfo, WindowOpenOptions, WindowScalePolicy,
};

use super::keyboard::{convert_key_press_event, convert_key_release_event, key_mods};

#[cfg(feature = "opengl")]
use crate::gl::{platform, GlContext};

pub struct WindowHandle {
    raw_window_handle: Option<RawWindowHandle>,
    close_requested: Arc<AtomicBool>,
    is_open: Arc<AtomicBool>,
}

impl WindowHandle {
    pub fn close(&mut self) {
        if self.raw_window_handle.take().is_some() {
            // FIXME: This will need to be changed from just setting an atomic to somehow
            // synchronizing with the window being closed (using a synchronous channel, or
            // by joining on the event loop thread).

            self.close_requested.store(true, Ordering::Relaxed);
        }
    }

    pub fn is_open(&self) -> bool {
        self.is_open.load(Ordering::Relaxed)
    }
}

unsafe impl HasRawWindowHandle for WindowHandle {
    fn raw_window_handle(&self) -> RawWindowHandle {
        if let Some(raw_window_handle) = self.raw_window_handle {
            if self.is_open.load(Ordering::Relaxed) {
                return raw_window_handle;
            }
        }

        RawWindowHandle::Xlib(XlibWindowHandle::empty())
    }
}

struct ParentHandle {
    close_requested: Arc<AtomicBool>,
    is_open: Arc<AtomicBool>,
}

impl ParentHandle {
    pub fn new() -> (Self, WindowHandle) {
        let close_requested = Arc::new(AtomicBool::new(false));
        let is_open = Arc::new(AtomicBool::new(true));

        let handle = WindowHandle {
            raw_window_handle: None,
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

struct WindowInner {
    xcb_connection: XcbConnection,
    window_id: XWindow,
    window_info: WindowInfo,
    visual_id: Visualid,
    mouse_cursor: MouseCursor,

    frame_interval: Duration,
    event_loop_running: bool,
    close_requested: bool,

    new_physical_size: Option<PhySize>,
    parent_handle: Option<ParentHandle>,

    #[cfg(feature = "opengl")]
    gl_context: Option<GlContext>,
}

pub struct Window<'a> {
    inner: &'a mut WindowInner,
}

// Hack to allow sending a RawWindowHandle between threads. Do not make public
struct SendableRwh(RawWindowHandle);

unsafe impl Send for SendableRwh {}

type WindowOpenResult = Result<SendableRwh, ()>;

impl<'a> Window<'a> {
    pub fn open_parented<P, H, B>(parent: &P, options: WindowOpenOptions, build: B) -> WindowHandle
    where
        P: HasRawWindowHandle,
        H: WindowHandler + 'static,
        B: FnOnce(&mut crate::Window) -> H,
        B: Send + 'static,
    {
        // Convert parent into something that X understands
        let parent_id = match parent.raw_window_handle() {
            RawWindowHandle::Xlib(h) => h.window as u32,
            RawWindowHandle::Xcb(h) => h.window,
            h => panic!("unsupported parent handle type {:?}", h),
        };

        let (tx, rx) = mpsc::sync_channel::<WindowOpenResult>(1);

        let (parent_handle, mut window_handle) = ParentHandle::new();

        thread::spawn(move || {
            Self::window_thread(Some(parent_id), options, build, tx.clone(), Some(parent_handle))
                .unwrap();
        });

        let raw_window_handle = rx.recv().unwrap().unwrap();
        window_handle.raw_window_handle = Some(raw_window_handle.0);

        window_handle
    }

    pub fn open_blocking<H, B>(options: WindowOpenOptions, build: B)
    where
        H: WindowHandler + 'static,
        B: FnOnce(&mut crate::Window) -> H,
        B: Send + 'static,
    {
        let (tx, rx) = mpsc::sync_channel::<WindowOpenResult>(1);

        let thread = thread::spawn(move || {
            Self::window_thread(None, options, build, tx, None).unwrap();
        });

        let _ = rx.recv().unwrap().unwrap();

        thread.join().unwrap_or_else(|err| {
            eprintln!("Window thread panicked: {:#?}", err);
        });
    }

    fn window_thread<H, B>(
        parent: Option<u32>, options: WindowOpenOptions, build: B,
        tx: mpsc::SyncSender<WindowOpenResult>, parent_handle: Option<ParentHandle>,
    ) -> Result<(), Box<dyn Error>>
    where
        H: WindowHandler + 'static,
        B: FnOnce(&mut crate::Window) -> H,
        B: Send + 'static,
    {
        // Connect to the X server
        // FIXME: baseview error type instead of unwrap()
        let xcb_connection = XcbConnection::new()?;

        // Get screen information (?)
        let setup = xcb_connection.conn2.setup();
        let screen = &setup.roots[xcb_connection.screen];

        let parent_id = parent.unwrap_or_else(|| screen.root);

        let gc_id = xcb_connection.conn2.generate_id()?;
        xcb_connection.conn2.create_gc(
            gc_id,
            parent_id,
            &CreateGCAux::new().foreground(screen.black_pixel).graphics_exposures(0),
        )?;

        let scaling = match options.scale {
            WindowScalePolicy::SystemScaleFactor => xcb_connection.get_scaling().unwrap_or(1.0),
            WindowScalePolicy::ScaleFactor(scale) => scale,
        };

        let window_info = WindowInfo::from_logical_size(options.size, scaling);

        // Now it starts becoming fun. If we're creating an OpenGL context, then we need to create
        // the window with a visual that matches the framebuffer used for the OpenGL context. So the
        // idea is that we first retrieve a framebuffer config that matches our wanted OpenGL
        // configuration, find the visual that matches that framebuffer config, create the window
        // with that visual, and then finally create an OpenGL context for the window. If we don't
        // use OpenGL, then we'll just take a random visual with a 32-bit depth.
        let create_default_config = || {
            Self::find_visual_for_depth(screen, 32)
                .map(|visual| (32, visual))
                .unwrap_or((x11rb::COPY_FROM_PARENT as u8, x11rb::COPY_FROM_PARENT as u32))
        };
        #[cfg(feature = "opengl")]
        let (fb_config, (depth, visual)) = match options.gl_config {
            Some(gl_config) => unsafe {
                platform::GlContext::get_fb_config_and_visual(xcb_connection.dpy, gl_config)
                    .map(|(fb_config, window_config)| {
                        (Some(fb_config), (window_config.depth, window_config.visual))
                    })
                    .expect("Could not fetch framebuffer config")
            },
            None => (None, create_default_config()),
        };
        #[cfg(not(feature = "opengl"))]
        let (depth, visual) = create_default_config();

        // For this 32-bith depth to work, you also need to define a color map and set a border
        // pixel: https://cgit.freedesktop.org/xorg/xserver/tree/dix/window.c#n818
        let colormap = xcb_connection.conn2.generate_id()?;
        xcb_connection.conn2.create_colormap(ColormapAlloc::NONE, colormap, screen.root, visual)?;

        let window_id = xcb_connection.conn2.generate_id()?;
        xcb_connection.conn2.create_window(
            depth,
            window_id,
            parent_id,
            0,                                         // x coordinate of the new window
            0,                                         // y coordinate of the new window
            window_info.physical_size().width as u16,  // window width
            window_info.physical_size().height as u16, // window height
            0,                                         // window border
            WindowClass::INPUT_OUTPUT,
            visual,
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
                        | EventMask::LEAVE_WINDOW,
                )
                // As mentioned above, these two values are needed to be able to create a window
                // with a depth of 32-bits when the parent window has a different depth
                .colormap(colormap)
                .border_pixel(0),
        )?;
        xcb_connection.conn2.map_window(window_id)?;

        // Change window title
        let title = options.title;
        xcb_connection.conn2.change_property8(
            PropMode::REPLACE,
            window_id,
            AtomEnum::WM_NAME,
            AtomEnum::STRING,
            title.as_bytes(),
        )?;

        xcb_connection.conn2.change_property32(
            PropMode::REPLACE,
            window_id,
            xcb_connection.atoms2.WM_PROTOCOLS,
            AtomEnum::ATOM,
            &[xcb_connection.atoms2.WM_DELETE_WINDOW],
        )?;

        xcb_connection.conn2.flush()?;

        // TODO: These APIs could use a couple tweaks now that everything is internal and there is
        //       no error handling anymore at this point. Everything is more or less unchanged
        //       compared to when raw-gl-context was a separate crate.
        #[cfg(feature = "opengl")]
        let gl_context = fb_config.map(|fb_config| {
            use std::ffi::c_ulong;

            let window = window_id as c_ulong;
            let display = xcb_connection.dpy;

            // Because of the visual negotation we had to take some extra steps to create this context
            let context = unsafe { platform::GlContext::create(window, display, fb_config) }
                .expect("Could not create OpenGL context");
            GlContext::new(context)
        });

        let mut inner = WindowInner {
            xcb_connection,
            window_id,
            window_info,
            visual_id: visual,
            mouse_cursor: MouseCursor::default(),

            frame_interval: Duration::from_millis(15),
            event_loop_running: false,
            close_requested: false,

            new_physical_size: None,
            parent_handle,

            #[cfg(feature = "opengl")]
            gl_context,
        };

        let mut window = crate::Window::new(Window { inner: &mut inner });

        let mut handler = build(&mut window);

        // Send an initial window resized event so the user is alerted of
        // the correct dpi scaling.
        handler.on_event(&mut window, Event::Window(WindowEvent::Resized(window_info)));

        let _ = tx.send(Ok(SendableRwh(window.raw_window_handle())));

        inner.run_event_loop(&mut handler)?;

        Ok(())
    }

    pub fn set_mouse_cursor(&mut self, mouse_cursor: MouseCursor) {
        if self.inner.mouse_cursor == mouse_cursor {
            return;
        }

        let xid = self.inner.xcb_connection.get_cursor(mouse_cursor).unwrap();

        if xid != 0 {
            let _ = self.inner.xcb_connection.conn2.change_window_attributes(
                self.inner.window_id,
                &ChangeWindowAttributesAux::new().cursor(xid),
            );
            let _ = self.inner.xcb_connection.conn2.flush();
        }

        self.inner.mouse_cursor = mouse_cursor;
    }

    pub fn close(&mut self) {
        self.inner.close_requested = true;
    }

    pub fn resize(&mut self, size: Size) {
        let scaling = self.inner.window_info.scale();
        let new_window_info = WindowInfo::from_logical_size(size, scaling);

        let _ = self.inner.xcb_connection.conn2.configure_window(
            self.inner.window_id,
            &ConfigureWindowAux::new()
                .width(new_window_info.physical_size().width)
                .height(new_window_info.physical_size().height),
        );
        let _ = self.inner.xcb_connection.conn2.flush();

        // This will trigger a `ConfigureNotify` event which will in turn change `self.window_info`
        // and notify the window handler about it
    }

    #[cfg(feature = "opengl")]
    pub fn gl_context(&self) -> Option<&crate::gl::GlContext> {
        self.inner.gl_context.as_ref()
    }

    fn find_visual_for_depth(screen: &Screen, depth: u8) -> Option<Visualid> {
        for candidate_depth in &screen.allowed_depths {
            if candidate_depth.depth != depth {
                continue;
            }

            for candidate_visual in &candidate_depth.visuals {
                if candidate_visual.class == VisualClass::TRUE_COLOR {
                    return Some(candidate_visual.visual_id);
                }
            }
        }

        None
    }
}

impl WindowInner {
    #[inline]
    fn drain_xcb_events(&mut self, handler: &mut dyn WindowHandler) -> Result<(), Box<dyn Error>> {
        // the X server has a tendency to send spurious/extraneous configure notify events when a
        // window is resized, and we need to batch those together and just send one resize event
        // when they've all been coalesced.
        self.new_physical_size = None;

        while let Some(event) = self.xcb_connection.conn2.poll_for_event()? {
            self.handle_xcb_event(handler, event);
        }

        if let Some(size) = self.new_physical_size.take() {
            self.window_info = WindowInfo::from_physical_size(size, self.window_info.scale());

            let window_info = self.window_info;

            handler.on_event(
                &mut crate::Window::new(Window { inner: self }),
                Event::Window(WindowEvent::Resized(window_info)),
            );
        }

        Ok(())
    }

    // Event loop
    // FIXME: poll() acts fine on linux, sometimes funky on *BSD. XCB upstream uses a define to
    // switch between poll() and select() (the latter of which is fine on *BSD), and we should do
    // the same.
    fn run_event_loop(&mut self, handler: &mut dyn WindowHandler) -> Result<(), Box<dyn Error>> {
        use nix::poll::*;

        let xcb_fd = self.xcb_connection.conn2.as_raw_fd();

        let mut last_frame = Instant::now();
        self.event_loop_running = true;

        while self.event_loop_running {
            // We'll try to keep a consistent frame pace. If the last frame couldn't be processed in
            // the expected frame time, this will throttle down to prevent multiple frames from
            // being queued up. The conditional here is needed because event handling and frame
            // drawing is interleaved. The `poll()` function below will wait until the next frame
            // can be drawn, or until the window receives an event. We thus need to manually check
            // if it's already time to draw a new frame.
            let next_frame = last_frame + self.frame_interval;
            if Instant::now() >= next_frame {
                handler.on_frame(&mut crate::Window::new(Window { inner: self }));
                last_frame = Instant::max(next_frame, Instant::now() - self.frame_interval);
            }

            let mut fds = [PollFd::new(xcb_fd, PollFlags::POLLIN)];

            // Check for any events in the internal buffers
            // before going to sleep:
            self.drain_xcb_events(handler)?;

            // FIXME: handle errors
            poll(&mut fds, next_frame.duration_since(Instant::now()).subsec_millis() as i32)
                .unwrap();

            if let Some(revents) = fds[0].revents() {
                if revents.contains(PollFlags::POLLERR) {
                    panic!("xcb connection poll error");
                }

                if revents.contains(PollFlags::POLLIN) {
                    self.drain_xcb_events(handler)?;
                }
            }

            // Check if the parents's handle was dropped (such as when the host
            // requested the window to close)
            //
            // FIXME: This will need to be changed from just setting an atomic to somehow
            // synchronizing with the window being closed (using a synchronous channel, or
            // by joining on the event loop thread).
            if let Some(parent_handle) = &self.parent_handle {
                if parent_handle.parent_did_drop() {
                    self.handle_must_close(handler);
                    self.close_requested = false;
                }
            }

            // Check if the user has requested the window to close
            if self.close_requested {
                self.handle_must_close(handler);
                self.close_requested = false;
            }
        }

        Ok(())
    }

    fn handle_close_requested(&mut self, handler: &mut dyn WindowHandler) {
        handler.on_event(
            &mut crate::Window::new(Window { inner: self }),
            Event::Window(WindowEvent::WillClose),
        );

        // FIXME: handler should decide whether window stays open or not
        self.event_loop_running = false;
    }

    fn handle_must_close(&mut self, handler: &mut dyn WindowHandler) {
        handler.on_event(
            &mut crate::Window::new(Window { inner: self }),
            Event::Window(WindowEvent::WillClose),
        );

        self.event_loop_running = false;
    }

    fn handle_xcb_event(&mut self, handler: &mut dyn WindowHandler, event: XEvent) {
        // For all of the keyboard and mouse events, you can fetch
        // `x`, `y`, `detail`, and `state`.
        // - `x` and `y` are the position inside the window where the cursor currently is
        //   when the event happened.
        // - `detail` will tell you which keycode was pressed/released (for keyboard events)
        //   or which mouse button was pressed/released (for mouse events).
        //   For mouse events, here's what the value means (at least on my current mouse):
        //      1 = left mouse button
        //      2 = middle mouse button (scroll wheel)
        //      3 = right mouse button
        //      4 = scroll wheel up
        //      5 = scroll wheel down
        //      8 = lower side button ("back" button)
        //      9 = upper side button ("forward" button)
        //   Note that you *will* get a "button released" event for even the scroll wheel
        //   events, which you can probably ignore.
        // - `state` will tell you the state of the main three mouse buttons and some of
        //   the keyboard modifier keys at the time of the event.
        //   http://rtbo.github.io/rust-xcb/src/xcb/ffi/xproto.rs.html#445

        match event {
            ////
            // window
            ////
            XEvent::ClientMessage(event) => {
                if event.format == 32
                    && event.data.as_data32()[0] == self.xcb_connection.atoms2.WM_DELETE_WINDOW
                {
                    self.handle_close_requested(handler);
                }
            }

            XEvent::ConfigureNotify(event) => {
                let new_physical_size = PhySize::new(event.width as u32, event.height as u32);

                if self.new_physical_size.is_some()
                    || new_physical_size != self.window_info.physical_size()
                {
                    self.new_physical_size = Some(new_physical_size);
                }
            }

            ////
            // mouse
            ////
            XEvent::MotionNotify(event) => {
                let physical_pos = PhyPoint::new(event.event_x as i32, event.event_y as i32);
                let logical_pos = physical_pos.to_logical(&self.window_info);

                handler.on_event(
                    &mut crate::Window::new(Window { inner: self }),
                    Event::Mouse(MouseEvent::CursorMoved {
                        position: logical_pos,
                        modifiers: key_mods(event.state),
                    }),
                );
            }

            XEvent::EnterNotify(event) => {
                handler.on_event(
                    &mut crate::Window::new(Window { inner: self }),
                    Event::Mouse(MouseEvent::CursorEntered),
                );
                // since no `MOTION_NOTIFY` event is generated when `ENTER_NOTIFY` is generated,
                // we generate a CursorMoved as well, so the mouse position from here isn't lost
                let physical_pos = PhyPoint::new(event.event_x as i32, event.event_y as i32);
                let logical_pos = physical_pos.to_logical(&self.window_info);
                handler.on_event(
                    &mut crate::Window::new(Window { inner: self }),
                    Event::Mouse(MouseEvent::CursorMoved {
                        position: logical_pos,
                        modifiers: key_mods(event.state),
                    }),
                );
            }

            XEvent::LeaveNotify(_) => {
                handler.on_event(
                    &mut crate::Window::new(Window { inner: self }),
                    Event::Mouse(MouseEvent::CursorLeft),
                );
            }

            XEvent::ButtonPress(event) => match event.detail {
                4..=7 => {
                    handler.on_event(
                        &mut crate::Window::new(Window { inner: self }),
                        Event::Mouse(MouseEvent::WheelScrolled {
                            delta: match event.detail {
                                4 => ScrollDelta::Lines { x: 0.0, y: 1.0 },
                                5 => ScrollDelta::Lines { x: 0.0, y: -1.0 },
                                6 => ScrollDelta::Lines { x: -1.0, y: 0.0 },
                                7 => ScrollDelta::Lines { x: 1.0, y: 0.0 },
                                _ => unreachable!(),
                            },
                            modifiers: key_mods(event.state),
                        }),
                    );
                }
                detail => {
                    let button_id = mouse_id(detail);
                    handler.on_event(
                        &mut crate::Window::new(Window { inner: self }),
                        Event::Mouse(MouseEvent::ButtonPressed {
                            button: button_id,
                            modifiers: key_mods(event.state),
                        }),
                    );
                }
            },

            XEvent::ButtonRelease(event) => {
                if !(4..=7).contains(&event.detail) {
                    let button_id = mouse_id(event.detail);
                    handler.on_event(
                        &mut crate::Window::new(Window { inner: self }),
                        Event::Mouse(MouseEvent::ButtonReleased {
                            button: button_id,
                            modifiers: key_mods(event.state),
                        }),
                    );
                }
            }

            ////
            // keys
            ////
            XEvent::KeyPress(event) => {
                handler.on_event(
                    &mut crate::Window::new(Window { inner: self }),
                    Event::Keyboard(convert_key_press_event(&event)),
                );
            }

            XEvent::KeyRelease(event) => {
                handler.on_event(
                    &mut crate::Window::new(Window { inner: self }),
                    Event::Keyboard(convert_key_release_event(&event)),
                );
            }

            _ => {}
        }
    }
}

unsafe impl<'a> HasRawWindowHandle for Window<'a> {
    fn raw_window_handle(&self) -> RawWindowHandle {
        let mut handle = XlibWindowHandle::empty();

        handle.window = self.inner.window_id.into();
        handle.visual_id = self.inner.visual_id.into();

        RawWindowHandle::Xlib(handle)
    }
}

unsafe impl<'a> HasRawDisplayHandle for Window<'a> {
    fn raw_display_handle(&self) -> RawDisplayHandle {
        let display = self.inner.xcb_connection.dpy;
        let mut handle = XlibDisplayHandle::empty();

        handle.display = display as *mut c_void;
        handle.screen = unsafe { x11::xlib::XDefaultScreen(display) };

        RawDisplayHandle::Xlib(handle)
    }
}

fn mouse_id(id: u8) -> MouseButton {
    match id {
        1 => MouseButton::Left,
        2 => MouseButton::Middle,
        3 => MouseButton::Right,
        8 => MouseButton::Back,
        9 => MouseButton::Forward,
        id => MouseButton::Other(id),
    }
}

pub fn copy_to_clipboard(_data: &str) {
    todo!()
}
