use super::*;
use crate::handler::WindowHandlerBuilder;
use crate::platform::x11::event_loop::EventLoop;
use crate::platform::x11::window_shared::WindowInner;
use crate::{WindowContext, WindowOpenOptions};
use calloop::LoopSignal;
use std::cell::Cell;
use std::num::NonZeroU32;
use std::panic::resume_unwind;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Mutex};
use std::thread;
use std::thread::JoinHandle;

pub(crate) struct WindowThreadShared {
    stopped: AtomicBool,
    final_error: Mutex<Option<String>>,
}

impl WindowThreadShared {
    pub fn new() -> Self {
        Self { stopped: false.into(), final_error: None.into() }
    }
}

pub struct WindowThreadHandle {
    shared: Arc<WindowThreadShared>,
    loop_signal: LoopSignal,
    event_loop_handle: Cell<Option<JoinHandle<()>>>,
}

impl WindowThreadHandle {
    pub fn create_window(
        options: WindowOpenOptions, handler: WindowHandlerBuilder,
    ) -> Result<Self> {
        let (tx, rx) = result_channel();
        let shared = Arc::new(WindowThreadShared::new());

        let join_handle = {
            let shared = shared.clone();

            thread::spawn(move || {
                let thread = match WindowThread::create(options, handler, shared) {
                    Err(e) => return tx.send_error(e),
                    Ok(thread) => thread,
                };

                if tx.send_success(&thread) {
                    thread.run()
                }
            })
        };

        rx.receive(join_handle, shared)
    }

    pub fn run_until_closed(&self) -> Result<()> {
        let Some(thread) = self.event_loop_handle.take() else { return Ok(()) };

        if let Err(panic) = thread.join() {
            resume_unwind(panic);
        }

        if let Some(e) = self.shared.final_error.lock().unwrap().take() {
            return Err(Error::RunError(e));
        }

        Ok(())
    }

    pub fn is_open(&self) -> bool {
        !self.shared.stopped.load(Ordering::Relaxed)
    }
}

impl Drop for WindowThreadHandle {
    fn drop(&mut self) {
        self.loop_signal.stop();
        self.loop_signal.wakeup();

        self.run_until_closed();
    }
}

enum WindowOpenResult {
    Success { loop_signal: LoopSignal },
    Error(String),
}

struct WindowThread {
    event_loop: EventLoop,
    ev_loop: calloop::EventLoop<'static, EventLoop>,
    shared: Arc<WindowThreadShared>,
}

impl WindowThread {
    pub fn create(
        options: WindowOpenOptions, handler: WindowHandlerBuilder, shared: Arc<WindowThreadShared>,
    ) -> Result<Self> {
        let mut ev_loop = calloop::EventLoop::try_new()?;
        let inner = WindowInner::create(options, &ev_loop)?;
        let handler = handler.build(WindowContext::new(Rc::clone(&inner)))?;
        let event_loop = EventLoop::new(inner, handler, shared.clone(), &mut ev_loop)?;

        Ok(Self { event_loop, ev_loop, shared })
    }

    pub fn run(self) {
        if let Err(e) = self.event_loop.run(self.ev_loop) {
            self.shared.final_error.lock().unwrap().replace(e.to_string());
        }
    }
}

fn result_channel() -> (WindowResultSender, WindowResultReceiver) {
    let (tx, rx) = mpsc::sync_channel::<WindowOpenResult>(1);
    (WindowResultSender(tx), WindowResultReceiver(rx))
}

struct WindowResultSender(mpsc::SyncSender<WindowOpenResult>);
impl WindowResultSender {
    pub fn send_error(self, error: Error) {
        if let Err(err) = self.0.send(WindowOpenResult::Error(format!("{}", error))) {
            crate::error!("Window creation failed: {}", error);
            crate::warn!("Failed to send error to main thread: {}", err);
        }
    }

    pub fn send_success(self, thread: &WindowThread) -> bool {
        let msg = WindowOpenResult::Success { loop_signal: thread.ev_loop.get_signal() };

        if let Err(err) = self.0.send(msg) {
            crate::error!("Failed to send created window to main thread: {}. Aborting.", err);
            return false;
        }

        true
    }
}

struct WindowResultReceiver(mpsc::Receiver<WindowOpenResult>);
impl WindowResultReceiver {
    pub fn receive(
        self, join_handle: JoinHandle<()>, shared: Arc<WindowThreadShared>,
    ) -> Result<WindowThreadHandle> {
        let result = self.0.recv()?;
        match result {
            WindowOpenResult::Error(e) => Err(Error::CreationFailed(e)),
            WindowOpenResult::Success { loop_signal } => Ok(WindowThreadHandle {
                event_loop_handle: Some(join_handle).into(),
                shared,
                loop_signal,
            }),
        }
    }
}
