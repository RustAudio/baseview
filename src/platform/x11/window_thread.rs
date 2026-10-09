use super::*;
use crate::dpi::Size;
use crate::handler::WindowHandlerBuilder;
use crate::host::HostCallbacks;
use crate::platform::x11::event_loop::{EventLoop, MainThreadCaller};
use crate::platform::x11::present::PresentThreadShared;
use crate::platform::x11::sizing::SizingThreadShared;
use crate::platform::x11::window_shared::WindowShared;
use crate::utils::SizingStrategy;
use crate::warn;
use crate::window::WindowInitializer;
use crate::{WindowContext, WindowSettings, WindowSize};
use calloop::LoopSignal;
use std::cell::{Cell, RefCell};
use std::panic::resume_unwind;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Mutex};
use std::thread;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

pub struct WindowThreadShared {
    stopped: AtomicBool,

    pub sizing: SizingThreadShared,
    pub present: PresentThreadShared,

    final_error: Mutex<Option<String>>,
    stopped_requested_from_host: AtomicBool,
}

pub enum RedrawRequested {
    Now,
    Later(Instant),
}

impl RedrawRequested {
    pub fn from_duration(duration: Duration) -> Self {
        if duration.is_zero() {
            RedrawRequested::Now
        } else {
            if let Some(dur) = Instant::now().checked_add(duration) {
                RedrawRequested::Later(dur)
            } else {
                RedrawRequested::Now
            }
        }
    }

    pub fn to_duration(&self) -> Duration {
        match self {
            RedrawRequested::Now => Duration::ZERO,
            RedrawRequested::Later(instant) => {
                instant.checked_duration_since(Instant::now()).unwrap_or(Duration::ZERO)
            }
        }
    }
}

impl WindowThreadShared {
    pub fn new() -> Self {
        Self {
            stopped: false.into(),
            final_error: None.into(),
            stopped_requested_from_host: false.into(),
            present: PresentThreadShared::new(),
            sizing: SizingThreadShared::new(),
        }
    }

    fn init(&self, window: &WindowShared) {
        self.sizing.init(&window.sizing_state)
    }

    pub fn is_stop_host_requested(&self) -> bool {
        self.stopped_requested_from_host.load(Ordering::Relaxed)
    }
}

struct ThreadStopWatcher(Arc<WindowThreadShared>);

impl Drop for ThreadStopWatcher {
    fn drop(&mut self) {
        self.0.stopped.store(true, Ordering::Relaxed);
    }
}

pub enum WindowThreadRequest {
    SuggestScaleFactor(f64),
    Resize(Size),
    SetParent(ParentWindowHandle),
    Show,
    Hide,
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
    host_callbacks: Option<RefCell<Box<dyn HostCallbacks>>>,
}

impl WindowThreadHandle {
    pub fn create_window(mut init: WindowInitializer) -> PlatformResult<Self> {
        let (tx, rx) = result_channel();
        let shared = Arc::new(WindowThreadShared::new());
        let (request_sender, request_receiver) = calloop::channel::sync_channel(1);
        let (response_sender, response_receiver) = mpsc::channel();
        let (main_thread_caller, main_thread_receiver) =
            MainThreadCaller::new(init.host.take_main_thread());

        let join_handle = {
            let shared = Arc::clone(&shared);
            let stop_watcher = ThreadStopWatcher(Arc::clone(&shared));

            thread::spawn(move || {
                let thread = match WindowThread::create(
                    init.settings,
                    init.builder,
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

                // Forces the stop_watcher to be moved to this closure
                drop(stop_watcher);
            })
        };

        let loop_signal = rx.receive()?;

        Ok(WindowThreadHandle {
            event_loop_handle: Some(join_handle).into(),
            shared,
            loop_signal,
            request_sender,
            response_receiver,
            host_callbacks: init.host.take_callbacks().map(|c| c.into()),
            callback_receiver: main_thread_receiver,
        })
    }

    pub fn size(&self) -> WindowSize {
        self.shared.sizing.window_size()
    }

    pub fn resize(&self, size: Size) -> PlatformResult<()> {
        self.request(WindowThreadRequest::Resize(size))
    }

    pub fn suggest_scale_factor(&self, scale_factor: f64) -> PlatformResult<()> {
        self.request(WindowThreadRequest::SuggestScaleFactor(scale_factor))
    }

    fn request(&self, req: WindowThreadRequest) -> PlatformResult<()> {
        self.request_sender.send(req).map_err(|_| RequestFailed::Send)?;
        let result = self.response_receiver.recv().map_err(|_| RequestFailed::Recv)?;

        result.map_err(|e| RequestFailed::Response(e).into())
    }

    pub fn sizing_strategy(&self) -> SizingStrategy {
        self.shared.sizing.sizing_strategy()
    }

    pub fn run_until_closed(&self) -> PlatformResult<()> {
        if !self.shared.stopped.load(Ordering::Relaxed) {
            self.request(WindowThreadRequest::Show)?;
        }

        let Some(thread) = self.event_loop_handle.take() else { return Ok(()) };

        if let Err(panic) = thread.join() {
            resume_unwind(panic);
        }

        // Ignore poisoned mutex
        if let Some(e) = self.shared.final_error.lock().unwrap_or_else(|g| g.into_inner()).take() {
            return Err(PlatformError::Run(e));
        }

        Ok(())
    }

    pub fn show(&self) -> PlatformResult<()> {
        self.request(WindowThreadRequest::Show)
    }

    pub fn hide(&self) -> PlatformResult<()> {
        self.request(WindowThreadRequest::Hide)
    }

    pub fn is_open(&self) -> bool {
        !self.shared.stopped.load(Ordering::Relaxed)
    }

    pub fn is_resizable(&self) -> bool {
        self.sizing_strategy().is_resizable()
    }

    pub fn min_size(&self) -> Option<Size> {
        self.sizing_strategy().min_size()
    }

    pub fn max_size(&self) -> Option<Size> {
        self.sizing_strategy().max_size()
    }

    pub fn handle_main_thread_callback(&self) {
        loop {
            let Some(receiver) = self.callback_receiver.as_ref() else { return };
            let Some(callback) = receiver.try_recv().ok() else { return };

            self.handle_main_thread_message(callback);
        }
    }

    pub fn set_parent(&self, new_parent: ParentWindowHandle) -> PlatformResult<()> {
        self.request(WindowThreadRequest::SetParent(new_parent))
    }

    pub fn request_poll(&self) -> PlatformResult<()> {
        self.shared.present.request_poll();
        self.loop_signal.wakeup();

        Ok(())
    }

    pub fn waker(&self) -> WindowWaker {
        WindowWaker { loop_signal: self.loop_signal.clone(), shared: Arc::clone(&self.shared) }
    }

    fn handle_main_thread_message(&self, msg: HostCallback) {
        let Some(host_callbacks) = self.host_callbacks.as_ref() else { return };
        let mut host_callbacks = host_callbacks.borrow_mut();

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
        options: WindowSettings, handler: WindowHandlerBuilder, shared: Arc<WindowThreadShared>,
        receiver: calloop::channel::Channel<WindowThreadRequest>,
        sender: mpsc::Sender<WindowThreadResponseMessage>,
        main_thread_caller: Option<MainThreadCaller>,
    ) -> PlatformResult<Self> {
        let mut ev_loop = calloop::EventLoop::try_new()?;
        let parent_id = options.parent.as_ref().map(|p| p.inner.window_id);
        let inner = WindowShared::create(options, &ev_loop, Arc::clone(&shared))?;

        shared.init(&inner);

        let handler = handler.build(WindowContext::new(Rc::clone(&inner)))?;
        let event_loop = EventLoop::new(inner, handler, parent_id)?;

        Ok(Self { event_loop, ev_loop, shared })
    }

    pub fn run(self) {
        if let Err(e) = self.event_loop.run(self.ev_loop) {
            // Ignore a poisoned mutex, we just fully override this value anyway.
            let mut guard = self.shared.final_error.lock().unwrap_or_else(|g| g.into_inner());

            guard.replace(e.to_string());
        }
    }
}

fn result_channel() -> (WindowResultSender, WindowResultReceiver) {
    let (tx, rx) = mpsc::sync_channel::<WindowOpenResult>(1);
    (WindowResultSender(tx), WindowResultReceiver(rx))
}

struct WindowResultSender(mpsc::SyncSender<WindowOpenResult>);
impl WindowResultSender {
    pub fn send_error(self, error: PlatformError) {
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
    pub fn receive(self) -> PlatformResult<LoopSignal> {
        let result = self.0.recv().map_err(|_| PlatformError::MainThreadRecvResult)?;

        match result {
            WindowOpenResult::Error(e) => Err(PlatformError::CreationFailed(e)),
            WindowOpenResult::Success { loop_signal } => Ok(loop_signal),
        }
    }
}
