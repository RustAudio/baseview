use super::*;
use crate::handler::WindowHandlerBuilder;
use crate::host::{Host, HostCallbacks};
use crate::platform::x11::event_loop::{EventLoop, MainThreadCaller};
use crate::platform::x11::window_shared::WindowInner;
use crate::warn;
use crate::{WindowContext, WindowOpenOptions, WindowSize};
use calloop::LoopSignal;
use dpi::{PhysicalSize, Size};
use std::cell::Cell;
use std::panic::resume_unwind;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::{mpsc, Mutex};
use std::thread;
use std::thread::JoinHandle;

pub(crate) struct WindowThreadShared {
    stopped: AtomicBool,
    scaling_factor: AtomicU64,
    size: AtomicU32,
    final_error: Mutex<Option<String>>,
    stopped_requested_from_host: AtomicBool,
}

impl WindowThreadShared {
    pub fn new() -> Self {
        Self {
            stopped: false.into(),
            final_error: None.into(),
            size: 0.into(),
            scaling_factor: 0.into(),
            stopped_requested_from_host: false.into(),
        }
    }

    fn init(&self, window: &WindowInner) {
        self.set_size(window.get_size());
        self.set_scaling_factor(window.scale_factor());
    }

    pub fn get_size(&self) -> PhysicalSize<u16> {
        let bytes = self.size.load(Ordering::Relaxed);
        let low = (bytes & u16::MAX as u32) as u16;
        let high = (bytes >> 16) as u16;

        PhysicalSize::new(low, high)
    }

    pub fn set_size(&self, size: PhysicalSize<u16>) {
        let bytes = ((size.height as u32) << 16) | (size.width as u32);
        self.size.store(bytes, Ordering::Relaxed);
    }

    pub fn get_scaling_factor(&self) -> f64 {
        f64::from_be_bytes(self.scaling_factor.load(Ordering::Relaxed).to_ne_bytes())
    }

    pub fn set_scaling_factor(&self, scale_factor: f64) {
        self.scaling_factor
            .store(u64::from_be_bytes(scale_factor.to_ne_bytes()), Ordering::Relaxed);
    }

    pub fn is_stop_host_requested(&self) -> bool {
        self.stopped_requested_from_host.load(Ordering::Relaxed)
    }
}

pub enum WindowThreadRequest {
    SuggestScaleFactor(f64),
    Resize(Size),
    SetParent(ParentWindowHandle),
}

pub type WindowThreadResponseMessage = core::result::Result<(), String>;

pub enum HostCallback {
    Resized { new_size: WindowSize, previous: WindowSize },
    Destroyed,
}

pub struct WindowThreadHandle {
    shared: Arc<WindowThreadShared>,
    loop_signal: LoopSignal,
    event_loop_handle: Cell<Option<JoinHandle<()>>>,

    request_sender: calloop::channel::SyncSender<WindowThreadRequest>,
    response_receiver: mpsc::Receiver<WindowThreadResponseMessage>,
    callback_receiver: Option<mpsc::Receiver<HostCallback>>,
    host_callbacks: Option<Box<dyn HostCallbacks>>,
}

impl WindowThreadHandle {
    pub fn create_window(
        options: WindowOpenOptions, handler: WindowHandlerBuilder, host: Host,
    ) -> Result<Self> {
        let (tx, rx) = result_channel();
        let shared = Arc::new(WindowThreadShared::new());
        let (request_sender, request_receiver) = calloop::channel::sync_channel(1);
        let (response_sender, response_receiver) = mpsc::channel();
        let (main_thread_caller, main_thread_receiver) = MainThreadCaller::new(host.main_thread);

        let join_handle = {
            let shared = shared.clone();

            thread::spawn(move || {
                let thread = match WindowThread::create(
                    options,
                    handler,
                    shared,
                    request_receiver,
                    response_sender,
                    main_thread_caller,
                ) {
                    Err(e) => return tx.send_error(e),
                    Ok(thread) => thread,
                };

                if tx.send_success(&thread) {
                    thread.run()
                }
            })
        };

        let loop_signal = rx.receive()?;

        Ok(WindowThreadHandle {
            event_loop_handle: Some(join_handle).into(),
            shared,
            loop_signal,
            request_sender,
            response_receiver,
            host_callbacks: host.callbacks.map(|c| c.into_inner()),
            callback_receiver: main_thread_receiver,
        })
    }

    pub fn size(&self) -> WindowSize {
        let scale_factor = self.shared.get_scaling_factor();
        let size = self.shared.get_size();

        WindowSize::from_physical(size.cast(), scale_factor)
    }

    pub fn resize(&self, size: Size) -> Result<()> {
        self.request(WindowThreadRequest::Resize(size))
    }

    pub fn suggest_scale_factor(&self, scale_factor: f64) -> Result<()> {
        self.request(WindowThreadRequest::SuggestScaleFactor(scale_factor))
    }

    fn request(&self, req: WindowThreadRequest) -> Result<()> {
        self.request_sender.send(req).map_err(|_| RequestFailed::Send)?;
        let result = self.response_receiver.recv().map_err(|_| RequestFailed::Recv)?;

        result.map_err(|e| RequestFailed::Response(e).into())
    }

    pub fn run_until_closed(&self) -> Result<()> {
        let Some(thread) = self.event_loop_handle.take() else { return Ok(()) };

        if let Err(panic) = thread.join() {
            resume_unwind(panic);
        }

        if let Some(e) = self.shared.final_error.lock().unwrap().take() {
            return Err(Error::Run(e));
        }

        Ok(())
    }

    pub fn is_open(&self) -> bool {
        !self.shared.stopped.load(Ordering::Relaxed)
    }

    pub fn handle_main_thread_callback(&mut self) {
        loop {
            let Some(receiver) = self.callback_receiver.as_mut() else { return };
            let Some(callback) = receiver.try_recv().ok() else { return };

            self.handle_main_thread_message(callback);
        }
    }

    pub fn set_parent(&self, new_parent: ParentWindowHandle) -> Result<()> {
        self.request(WindowThreadRequest::SetParent(new_parent))
    }

    fn handle_main_thread_message(&mut self, msg: HostCallback) {
        let Some(host_callbacks) = self.host_callbacks.as_mut() else { return };

        match msg {
            HostCallback::Destroyed => host_callbacks.destroyed(),
            HostCallback::Resized { new_size: new, previous } => {
                if let Err(e) = host_callbacks.request_resize(new) {
                    warn!("Host failed to resize parent window: {}. Reverting.", e);

                    if let Err(e) = self.resize(previous.physical.into()) {
                        warn!(
                            "Failed to revert to previous size while handling previous error: {}",
                            e
                        );
                    }
                }
            }
        }
    }
}

impl Drop for WindowThreadHandle {
    fn drop(&mut self) {
        self.shared.stopped_requested_from_host.store(true, Ordering::Relaxed);
        self.loop_signal.stop();
        self.loop_signal.wakeup();

        if let Err(e) = self.run_until_closed() {
            warn!("Error while closing window: {}", e)
        }
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

#[derive(Debug)]
pub enum RequestFailed {
    Send,
    Recv,
    Response(String),
}

impl Display for RequestFailed {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            RequestFailed::Send => f.write_str("Request to X11 thread failed: Could not send request (X11 thread disconnected)"),
            RequestFailed::Recv => f.write_str("Request to X11 thread failed: Could not receive response (X11 thread disconnected)"),
            RequestFailed::Response(e) => f.write_str(e),
        }
    }
}

impl WindowThread {
    pub fn create(
        options: WindowOpenOptions, handler: WindowHandlerBuilder, shared: Arc<WindowThreadShared>,
        receiver: calloop::channel::Channel<WindowThreadRequest>,
        sender: mpsc::Sender<WindowThreadResponseMessage>,
        main_thread_caller: Option<MainThreadCaller>,
    ) -> Result<Self> {
        let mut ev_loop = calloop::EventLoop::try_new()?;
        let inner = WindowInner::create(options, &ev_loop, shared.clone())?;

        shared.init(&inner);

        let handler = handler.build(WindowContext::new(Rc::clone(&inner)))?;
        let event_loop =
            EventLoop::new(inner, handler, receiver, sender, main_thread_caller, &mut ev_loop)?;

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
    pub fn receive(self) -> Result<LoopSignal> {
        let result = self.0.recv().map_err(|_| Error::MainThreadRecvResult)?;

        match result {
            WindowOpenResult::Error(e) => Err(Error::CreationFailed(e)),
            WindowOpenResult::Success { loop_signal } => Ok(loop_signal),
        }
    }
}
