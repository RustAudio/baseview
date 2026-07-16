use std::cell::Cell;
use std::num::NonZero;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::Arc;
use std::thread::{self, JoinHandle};

use crate::handler::WindowHandlerBuilder;
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
            Err(e) => return Err(super::Error::Channel(e)),
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

pub fn copy_to_clipboard(_data: &str) {
    todo!()
}
