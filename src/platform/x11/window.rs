use std::cell::Cell;
use std::num::NonZero;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::Arc;
use std::thread::{self, JoinHandle};

use super::X11Connection;
use super::{event_loop::EventLoop, visual_info::WindowVisualConfig};
use crate::handler::WindowHandlerBuilder;
use crate::platform::x11::window_shared::WindowInner;
use crate::platform::x11::xcb_window::XcbWindow;
use crate::platform::Result;
use crate::*;

pub struct WindowHandle {
    window_id: Option<NonZero<x11rb::protocol::xproto::Window>>,
    event_loop_handle: Cell<Option<JoinHandle<()>>>,
    close_requested: Arc<AtomicBool>,
    is_open: Arc<AtomicBool>,
}

impl WindowHandle {
    pub fn create_window(
        options: WindowOpenOptions, handler: WindowHandlerBuilder,
    ) -> Result<WindowHandle> {
        let (tx, rx) = mpsc::sync_channel::<WindowOpenResult>(1);
        let (parent_handle, mut window_handle) = ParentHandle::new();

        let join_handle =
            thread::spawn(move || match create_window(options, handler, Some(parent_handle)) {
                Ok(ev_loop) => {
                    tx.send(Ok(ev_loop.window_id())).unwrap();
                    ev_loop.run().unwrap();
                }
                Err(e) => {
                    tx.send(Err(format!("{}", e))).unwrap();
                }
            });

        let id = match rx.recv() {
            Ok(Ok(id)) => id,
            Err(e) => return Err(super::Error::ChannelError(e)),
            Ok(Err(s)) => return Err(super::error::Error::CreationFailed(s)),
        };

        window_handle.window_id = Some(id);
        window_handle.event_loop_handle = Some(join_handle).into();
        Ok(window_handle)
    }

    pub fn run_until_closed(self) {
        let Some(thread) = self.event_loop_handle.take() else { return };

        thread.join().unwrap_or_else(|err| {
            eprintln!("Window thread panicked: {:#?}", err);
        });
    }

    pub fn is_open(&self) -> bool {
        self.is_open.load(Ordering::Relaxed)
    }
}

impl Drop for WindowHandle {
    fn drop(&mut self) {
        self.close_requested.store(true, Ordering::Relaxed);
        if let Some(event_loop) = self.event_loop_handle.take() {
            let _ = event_loop.join();
        }
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

type WindowOpenResult = core::result::Result<NonZero<x11rb::protocol::xproto::Window>, String>;

fn create_window(
    options: WindowOpenOptions, build: WindowHandlerBuilder, parent_handle: Option<ParentHandle>,
) -> Result<EventLoop> {
    // Connect to the X server
    let xcb_connection = X11Connection::new()?;

    let scaling = match options.scale {
        WindowScalePolicy::SystemScaleFactor => xcb_connection.get_scaling(),
        WindowScalePolicy::ScaleFactor(scale) => scale,
    };

    let physical_size = options.size.to_physical(scaling);

    #[cfg(feature = "opengl")]
    let visual_info =
        WindowVisualConfig::find_best_visual_config_for_gl(&xcb_connection, options.gl_config)?;

    #[cfg(not(feature = "opengl"))]
    let visual_info = WindowVisualConfig::find_best_visual_config(&xcb_connection)?;

    let xcb_connection = Rc::new(xcb_connection);

    let x_window = XcbWindow::new(
        Rc::clone(&xcb_connection),
        physical_size,
        &visual_info,
        options.parent.map(|p| p.window_id),
    )?;

    let cookies = [
        x_window.map_window()?,
        x_window.set_title(&options.title)?,
        x_window.enable_wm_protocols()?,
        x_window.enable_dnd_protocols()?,
    ];

    for cookie in cookies {
        cookie.check()?;
    }

    #[cfg(feature = "opengl")]
    let gl_context = match visual_info.fb_config {
        None => None,
        Some(fb_config) => {
            // Because of the visual negotation we had to take some extra steps to create this context
            Some(super::gl::GlContextInner::create(
                &x_window,
                Rc::clone(&xcb_connection),
                fb_config,
            )?)
        }
    };

    let inner = Rc::new(WindowInner::new(
        xcb_connection,
        x_window,
        physical_size,
        scaling,
        visual_info.visual_id,
        #[cfg(feature = "opengl")]
        gl_context,
    ));

    let handler = build.build(WindowContext::new(Rc::clone(&inner)))?;

    Ok(EventLoop::new(inner, handler, parent_handle))
}

pub fn copy_to_clipboard(_data: &str) {
    todo!()
}
