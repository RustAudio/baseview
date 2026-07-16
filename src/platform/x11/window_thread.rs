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
use std::sync::mpsc;
use std::thread;
use std::thread::JoinHandle;
use std::time::Duration;

pub struct WindowThreadHandle {
    window_id: NonZeroU32,
    loop_signal: LoopSignal,
    event_loop_handle: Cell<Option<JoinHandle<()>>>,
}

impl WindowThreadHandle {
    pub fn create_window(
        options: WindowOpenOptions, handler: WindowHandlerBuilder,
    ) -> Result<Self> {
        let (tx, rx) = result_channel();

        let join_handle = thread::spawn(move || {
            let thread = match WindowThread::create(options, handler) {
                Err(e) => return tx.send_error(e),
                Ok(thread) => thread,
            };

            if tx.send_success(&thread) {
                thread.run()
            }
        });

        thread::sleep(Duration::from_millis(2000));

        rx.receive(join_handle)
    }

    pub fn run_until_closed(&self) {
        let Some(thread) = self.event_loop_handle.take() else { return };

        if let Err(panic) = thread.join() {
            resume_unwind(panic);
        }
    }

    pub fn is_open(&self) -> bool {
        todo!()
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
    Success { window_id: NonZeroU32, loop_signal: LoopSignal },
    Error(String),
}

struct WindowThread {
    event_loop: EventLoop,
    ev_loop: calloop::EventLoop<'static, EventLoop>,
}

impl WindowThread {
    pub fn create(options: WindowOpenOptions, handler: WindowHandlerBuilder) -> Result<Self> {
        let mut ev_loop = calloop::EventLoop::try_new()?;
        let inner = WindowInner::create(options, &ev_loop)?;
        let handler = handler.build(WindowContext::new(Rc::clone(&inner)))?;
        let event_loop = EventLoop::new(inner, handler, &mut ev_loop)?;

        Ok(Self { event_loop, ev_loop })
    }

    pub fn run(self) {
        self.event_loop.run(self.ev_loop).unwrap();
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
        let msg = WindowOpenResult::Success {
            loop_signal: thread.ev_loop.get_signal(),
            window_id: thread.event_loop.window_id(),
        };

        if let Err(err) = self.0.send(msg) {
            crate::error!("Failed to send created window to main thread: {}. Aborting.", err);
            return false;
        }

        true
    }
}

struct WindowResultReceiver(mpsc::Receiver<WindowOpenResult>);
impl WindowResultReceiver {
    pub fn receive(self, join_handle: JoinHandle<()>) -> Result<WindowThreadHandle> {
        let result = self.0.recv()?;
        match result {
            WindowOpenResult::Error(e) => Err(Error::CreationFailed(e)),
            WindowOpenResult::Success { window_id, loop_signal } => Ok(WindowThreadHandle {
                event_loop_handle: Some(join_handle).into(),
                window_id,
                loop_signal,
            }),
        }
    }
}
