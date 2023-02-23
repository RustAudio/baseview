use std::marker::PhantomData;
use std::os::raw::{c_ulong, c_void};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::Arc;
use std::thread;
use std::time::*;

use raw_window_handle::{HasRawWindowHandle, RawWindowHandle, XlibHandle};
use xcb::ffi::xcb_screen_t;
use xcb::StructPtr;

use super::XcbConnection;
use crate::{
    Event, MouseButton, MouseCursor, MouseEvent, PhyPoint, PhySize, ScrollDelta, Size, WindowEvent,
    WindowHandler, WindowInfo, WindowOpenOptions, WindowScalePolicy,
};

use super::keyboard::{convert_key_press_event, convert_key_release_event, key_mods};

#[cfg(feature = "opengl")]
use crate::{
    gl::{platform, GlContext},
    window::RawWindowHandleWrapper,
};

pub struct WindowHandle {
    raw_window_handle: Option<RawWindowHandle>,
    close_requested: Arc<AtomicBool>,
    is_open: Arc<AtomicBool>,

    // Ensure handle is !Send
    _phantom: PhantomData<*mut ()>,
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

        RawWindowHandle::Xlib(XlibHandle::empty())
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
            _phantom: PhantomData::default(),
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

pub struct Window {
    xcb_connection: XcbConnection,
    window_id: u32,
    window_info: WindowInfo,
    // FIXME: There's all this mouse cursor logic but it's never actually used, is this correct?
    mouse_cursor: MouseCursor,

    frame_interval: Duration,
    event_loop_running: bool,
    close_requested: bool,

    new_physical_size: Option<PhySize>,
    parent_handle: Option<ParentHandle>,

    #[cfg(feature = "opengl")]
    gl_context: Option<GlContext>,
}

// Hack to allow sending a RawWindowHandle between threads. Do not make public
struct SendableRwh(RawWindowHandle);

unsafe impl Send for SendableRwh {}

type WindowOpenResult = Result<SendableRwh, ()>;

impl Window {
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
            Self::window_thread(Some(parent_id), options, build, tx.clone(), Some(parent_handle));
        });

        let raw_window_handle = rx.recv().unwrap().unwrap();
        window_handle.raw_window_handle = Some(raw_window_handle.0);

        window_handle
    }

    pub fn open_as_if_parented<H, B>(options: WindowOpenOptions, build: B) -> WindowHandle
    where
        H: WindowHandler + 'static,
        B: FnOnce(&mut crate::Window) -> H,
        B: Send + 'static,
    {
        let (tx, rx) = mpsc::sync_channel::<WindowOpenResult>(1);

        let (parent_handle, mut window_handle) = ParentHandle::new();

        thread::spawn(move || {
            Self::window_thread(None, options, build, tx.clone(), Some(parent_handle));
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
            Self::window_thread(None, options, build, tx, None);
        });

        let _ = rx.recv().unwrap().unwrap();

        thread.join().unwrap_or_else(|err| {
            eprintln!("Window thread panicked: {:#?}", err);
        });
    }

    fn window_thread<H, B>(
        parent: Option<u32>, options: WindowOpenOptions, build: B,
        tx: mpsc::SyncSender<WindowOpenResult>, parent_handle: Option<ParentHandle>,
    ) where
        H: WindowHandler + 'static,
        B: FnOnce(&mut crate::Window) -> H,
        B: Send + 'static,
    {
        // Connect to the X server
        // FIXME: baseview error type instead of unwrap()
        let xcb_connection = XcbConnection::new().unwrap();

        // Get screen information (?)
        let setup = xcb_connection.conn.get_setup();
        let screen = setup.roots().nth(xcb_connection.xlib_display as usize).unwrap();

        let foreground = xcb_connection.conn.generate_id();

        let parent_id = parent.unwrap_or_else(|| screen.root());

        xcb::create_gc(
            &xcb_connection.conn,
            foreground,
            parent_id,
            &[(xcb::GC_FOREGROUND, screen.black_pixel()), (xcb::GC_GRAPHICS_EXPOSURES, 0)],
        );

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
            Self::find_visual_for_depth(&screen, 32)
                .map(|visual| (32, visual))
                .unwrap_or((xcb::COPY_FROM_PARENT as u8, xcb::COPY_FROM_PARENT as u32))
        };
        #[cfg(feature = "opengl")]
        let (fb_config, (depth, visual)) = match options.gl_config {
            Some(gl_config) => unsafe {
                platform::GlContext::get_fb_config_and_visual(
                    xcb_connection.conn.get_raw_dpy(),
                    gl_config,
                )
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
        let colormap = xcb_connection.conn.generate_id();
        xcb::create_colormap(
            &xcb_connection.conn,
            xcb::COLORMAP_ALLOC_NONE as u8,
            colormap,
            screen.root(),
            visual,
        );

        let window_id = xcb_connection.conn.generate_id();
        xcb::create_window_checked(
            &xcb_connection.conn,
            depth,
            window_id,
            parent_id,
            0,                                         // x coordinate of the new window
            0,                                         // y coordinate of the new window
            window_info.physical_size().width as u16,  // window width
            window_info.physical_size().height as u16, // window height
            0,                                         // window border
            xcb::WINDOW_CLASS_INPUT_OUTPUT as u16,
            visual,
            &[
                (
                    xcb::CW_EVENT_MASK,
                    xcb::EVENT_MASK_EXPOSURE
                        | xcb::EVENT_MASK_POINTER_MOTION
                        | xcb::EVENT_MASK_BUTTON_PRESS
                        | xcb::EVENT_MASK_BUTTON_RELEASE
                        | xcb::EVENT_MASK_KEY_PRESS
                        | xcb::EVENT_MASK_KEY_RELEASE
                        | xcb::EVENT_MASK_STRUCTURE_NOTIFY,
                ),
                // As mentioend above, these two values are needed to be able to create a window
                // with a dpeth of 32-bits when the parent window has a different depth
                (xcb::CW_COLORMAP, colormap),
                (xcb::CW_BORDER_PIXEL, 0),
            ],
        )
        .request_check()
        .unwrap();

        xcb::map_window(&xcb_connection.conn, window_id);

        // Change window title
        let title = options.title;
        xcb::change_property(
            &xcb_connection.conn,
            xcb::PROP_MODE_REPLACE as u8,
            window_id,
            xcb::ATOM_WM_NAME,
            xcb::ATOM_STRING,
            8, // view data as 8-bit
            title.as_bytes(),
        );

        if let Some((wm_protocols, wm_delete_window)) =
            xcb_connection.atoms.wm_protocols.zip(xcb_connection.atoms.wm_delete_window)
        {
            xcb_util::icccm::set_wm_protocols(
                &xcb_connection.conn,
                window_id,
                wm_protocols,
                &[wm_delete_window],
            );
        }

        xcb_connection.conn.flush();

        // TODO: These APIs could use a couple tweaks now that everything is internal and there is
        //       no error handling anymore at this point. Everything is more or less unchanged
        //       compared to when raw-gl-context was a separate crate.
        #[cfg(feature = "opengl")]
        let gl_context = fb_config.map(|fb_config| {
            let mut handle = XlibHandle::empty();
            handle.window = window_id as c_ulong;
            handle.display = xcb_connection.conn.get_raw_dpy() as *mut c_void;
            let handle = RawWindowHandleWrapper { handle: RawWindowHandle::Xlib(handle) };

            // Because of the visual negotation we had to take some extra steps to create this context
            let context = unsafe { platform::GlContext::create(&handle, fb_config) }
                .expect("Could not create OpenGL context");
            GlContext::new(context)
        });

        let mut window = Self {
            xcb_connection,
            window_id,
            window_info,
            mouse_cursor: MouseCursor::default(),

            frame_interval: Duration::from_millis(15),
            event_loop_running: false,
            close_requested: false,

            new_physical_size: None,
            parent_handle,

            #[cfg(feature = "opengl")]
            gl_context,
        };

        let mut handler = build(&mut crate::Window::new(&mut window));

        // Send an initial window resized event so the user is alerted of
        // the correct dpi scaling.
        handler.on_event(
            &mut crate::Window::new(&mut window),
            Event::Window(WindowEvent::Resized(window_info)),
        );

        let _ = tx.send(Ok(SendableRwh(window.raw_window_handle())));

        window.run_event_loop(&mut handler);
    }

    pub fn set_mouse_cursor(&mut self, mouse_cursor: MouseCursor) {
        if self.mouse_cursor == mouse_cursor {
            return;
        }

        let xid = self.xcb_connection.get_cursor_xid(mouse_cursor);

        if xid != 0 {
            xcb::change_window_attributes(
                &self.xcb_connection.conn,
                self.window_id,
                &[(xcb::CW_CURSOR, xid)],
            );

            self.xcb_connection.conn.flush();
        }

        self.mouse_cursor = mouse_cursor;
    }

    pub fn close(&mut self) {
        self.close_requested = true;
    }

    pub fn resize(&mut self, size: Size) {
        let scaling = self.window_info.scale();
        let new_window_info = WindowInfo::from_logical_size(size, scaling);

        xcb::configure_window(
            &self.xcb_connection.conn,
            self.window_id,
            &[
                (xcb::CONFIG_WINDOW_WIDTH as u16, new_window_info.physical_size().width),
                (xcb::CONFIG_WINDOW_HEIGHT as u16, new_window_info.physical_size().height),
            ],
        );
        self.xcb_connection.conn.flush();

        // This will trigger a `ConfigureNotify` event which will in turn change `self.window_info`
        // and notify the window handler about it
    }

    #[cfg(feature = "opengl")]
    pub fn gl_context(&self) -> Option<&crate::gl::GlContext> {
        self.gl_context.as_ref()
    }

    fn find_visual_for_depth(screen: &StructPtr<xcb_screen_t>, depth: u8) -> Option<u32> {
        for candidate_depth in screen.allowed_depths() {
            if candidate_depth.depth() != depth {
                continue;
            }

            for candidate_visual in candidate_depth.visuals() {
                if candidate_visual.class() == xcb::VISUAL_CLASS_TRUE_COLOR as u8 {
                    return Some(candidate_visual.visual_id());
                }
            }
        }

        None
    }

    #[inline]
    fn drain_xcb_events(&mut self, handler: &mut dyn WindowHandler) {
        // the X server has a tendency to send spurious/extraneous configure notify events when a
        // window is resized, and we need to batch those together and just send one resize event
        // when they've all been coalesced.
        self.new_physical_size = None;

        while let Some(event) = self.xcb_connection.conn.poll_for_event() {
            self.handle_xcb_event(handler, event);
        }

        if let Some(size) = self.new_physical_size.take() {
            self.window_info = WindowInfo::from_physical_size(size, self.window_info.scale());

            let window_info = self.window_info;

            handler.on_event(
                &mut crate::Window::new(self),
                Event::Window(WindowEvent::Resized(window_info)),
            );
        }
    }

    // Event loop
    // FIXME: poll() acts fine on linux, sometimes funky on *BSD. XCB upstream uses a define to
    // switch between poll() and select() (the latter of which is fine on *BSD), and we should do
    // the same.
    fn run_event_loop(&mut self, handler: &mut dyn WindowHandler) {
        use nix::poll::*;

        let xcb_fd = unsafe {
            let raw_conn = self.xcb_connection.conn.get_raw_conn();
            xcb::ffi::xcb_get_file_descriptor(raw_conn)
        };

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
                handler.on_frame(&mut crate::Window::new(self));
                last_frame = Instant::max(next_frame, Instant::now() - self.frame_interval);
            }

            let mut fds = [PollFd::new(xcb_fd, PollFlags::POLLIN)];

            // Check for any events in the internal buffers
            // before going to sleep:
            self.drain_xcb_events(handler);

            // FIXME: handle errors
            poll(&mut fds, next_frame.duration_since(Instant::now()).subsec_millis() as i32)
                .unwrap();

            if let Some(revents) = fds[0].revents() {
                if revents.contains(PollFlags::POLLERR) {
                    panic!("xcb connection poll error");
                }

                if revents.contains(PollFlags::POLLIN) {
                    self.drain_xcb_events(handler);
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
    }

    fn handle_close_requested(&mut self, handler: &mut dyn WindowHandler) {
        handler.on_event(&mut crate::Window::new(self), Event::Window(WindowEvent::WillClose));

        // FIXME: handler should decide whether window stays open or not
        self.event_loop_running = false;
    }

    fn handle_must_close(&mut self, handler: &mut dyn WindowHandler) {
        handler.on_event(&mut crate::Window::new(self), Event::Window(WindowEvent::WillClose));

        self.event_loop_running = false;
    }

    fn handle_xcb_event(&mut self, handler: &mut dyn WindowHandler, event: xcb::GenericEvent) {
        let event_type = event.response_type() & !0x80;

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

        match event_type {
            ////
            // window
            ////
            xcb::CLIENT_MESSAGE => {
                let event = unsafe { xcb::cast_event::<xcb::ClientMessageEvent>(&event) };

                // what an absolute tragedy this all is
                let data = event.data().data;
                let (_, data32, _) = unsafe { data.align_to::<u32>() };

                let wm_delete_window =
                    self.xcb_connection.atoms.wm_delete_window.unwrap_or(xcb::NONE);

                if wm_delete_window == data32[0] {
                    self.handle_close_requested(handler);
                }
            }

            xcb::CONFIGURE_NOTIFY => {
                let event = unsafe { xcb::cast_event::<xcb::ConfigureNotifyEvent>(&event) };

                let new_physical_size = PhySize::new(event.width() as u32, event.height() as u32);

                if self.new_physical_size.is_some()
                    || new_physical_size != self.window_info.physical_size()
                {
                    self.new_physical_size = Some(new_physical_size);
                }
            }

            ////
            // mouse
            ////
            xcb::MOTION_NOTIFY => {
                let event = unsafe { xcb::cast_event::<xcb::MotionNotifyEvent>(&event) };
                let detail = event.detail();

                if detail != 4 && detail != 5 {
                    let physical_pos =
                        PhyPoint::new(event.event_x() as i32, event.event_y() as i32);
                    let logical_pos = physical_pos.to_logical(&self.window_info);

                    handler.on_event(
                        &mut crate::Window::new(self),
                        Event::Mouse(MouseEvent::CursorMoved {
                            position: logical_pos,
                            modifiers: key_mods(event.state()),
                        }),
                    );
                }
            }

            xcb::BUTTON_PRESS => {
                let event = unsafe { xcb::cast_event::<xcb::ButtonPressEvent>(&event) };
                let detail = event.detail();

                match detail {
                    4..=7 => {
                        handler.on_event(
                            &mut crate::Window::new(self),
                            Event::Mouse(MouseEvent::WheelScrolled {
                                delta: match detail {
                                    4 => ScrollDelta::Lines { x: 0.0, y: 1.0 },
                                    5 => ScrollDelta::Lines { x: 0.0, y: -1.0 },
                                    6 => ScrollDelta::Lines { x: -1.0, y: 0.0 },
                                    7 => ScrollDelta::Lines { x: 1.0, y: 0.0 },
                                    _ => unreachable!(),
                                },
                                modifiers: key_mods(event.state()),
                            }),
                        );
                    }
                    detail => {
                        let button_id = mouse_id(detail);
                        handler.on_event(
                            &mut crate::Window::new(self),
                            Event::Mouse(MouseEvent::ButtonPressed {
                                button: button_id,
                                modifiers: key_mods(event.state()),
                            }),
                        );
                    }
                }
            }

            xcb::BUTTON_RELEASE => {
                let event = unsafe { xcb::cast_event::<xcb::ButtonPressEvent>(&event) };
                let detail = event.detail();

                if !(4..=7).contains(&detail) {
                    let button_id = mouse_id(detail);
                    handler.on_event(
                        &mut crate::Window::new(self),
                        Event::Mouse(MouseEvent::ButtonReleased {
                            button: button_id,
                            modifiers: key_mods(event.state()),
                        }),
                    );
                }
            }

            ////
            // keys
            ////
            xcb::KEY_PRESS => {
                let event = unsafe { xcb::cast_event::<xcb::KeyPressEvent>(&event) };

                handler.on_event(
                    &mut crate::Window::new(self),
                    Event::Keyboard(convert_key_press_event(event)),
                );
            }

            xcb::KEY_RELEASE => {
                let event = unsafe { xcb::cast_event::<xcb::KeyReleaseEvent>(&event) };

                handler.on_event(
                    &mut crate::Window::new(self),
                    Event::Keyboard(convert_key_release_event(event)),
                );
            }

            _ => {}
        }
    }
}

unsafe impl HasRawWindowHandle for Window {
    fn raw_window_handle(&self) -> RawWindowHandle {
        let mut handle = XlibHandle::empty();
        handle.window = self.window_id as c_ulong;
        handle.display = self.xcb_connection.conn.get_raw_dpy() as *mut c_void;

        RawWindowHandle::Xlib(handle)
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

pub fn copy_to_clipboard(data: &str) {
    todo!()
}
