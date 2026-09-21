use super::*;
use crate::dpi::{PhysicalSize, Size};
use crate::handler::WindowHandlerBuilder;
use crate::host::HostCallbacks;
use crate::platform::x11::event_loop::{EventLoop, MainThreadCaller};
use crate::platform::x11::window_shared::WindowInner;
use crate::utils::SizingStrategy;
use crate::warn;
use crate::window::WindowInitializer;
use crate::{WindowContext, WindowSettings, WindowSize};
use calloop::LoopSignal;
use std::cell::{Cell, RefCell};
use std::panic::resume_unwind;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::{mpsc, Mutex, OnceLock};
use std::thread;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

pub(crate) struct WindowThreadShared {
    stopped: AtomicBool,
    scaling_factor: AtomicU64,
    size: AtomicU32,
    final_error: Mutex<Option<String>>,
    stopped_requested_from_host: AtomicBool,
    poll_requested: AtomicBool,
    sizing_strategy: OnceLock<SizingStrategy>,

    redraw_requested_after: Mutex<Option<RedrawRequested>>,
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
            size: 0.into(),
            scaling_factor: 0.into(),
            stopped_requested_from_host: false.into(),
            sizing_strategy: OnceLock::new(),
            redraw_requested_after: None.into(),
            poll_requested: false.into(),
        }
    }

    fn init(&self, window: &WindowInner) {
        self.set_size(window.get_size());
        self.set_scaling_factor(window.scale_factor());
        let Ok(()) = self.sizing_strategy.set(window.sizing_strategy) else { unreachable!() };
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

    pub fn sizing_strategy(&self) -> SizingStrategy {
        self.sizing_strategy.get().copied().unwrap_or_default()
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

    pub fn request_redraw_after(&self, duration: Duration) {
        // Ignore a poisoned mutex, we just fully override this value anyway.
        let mut guard = self.redraw_requested_after.lock().unwrap_or_else(|g| g.into_inner());
        *guard = Some(RedrawRequested::from_duration(duration));
    }

    pub fn take_redraw_request(&self) -> Option<Duration> {
        let mut guard = self.redraw_requested_after.lock().unwrap_or_else(|g| g.into_inner());
        guard.take().map(|w| w.to_duration())
    }

    pub fn request_poll(&self) {
        self.poll_requested.store(true, Ordering::Relaxed);
    }

    pub fn take_poll_request(&self) -> bool {
        self.poll_requested.swap(false, Ordering::Relaxed)
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
    pub fn create_window(init: WindowInitializer) -> Result<Self> {
        let (tx, rx) = result_channel();
        let shared = Arc::new(WindowThreadShared::new());
        let (request_sender, request_receiver) = calloop::channel::sync_channel(1);
        let (response_sender, response_receiver) = mpsc::channel();
        let (main_thread_caller, main_thread_receiver) =
            MainThreadCaller::new(init.host.main_thread);

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
            host_callbacks: init.host.callbacks.map(|c| c.into_inner().into()),
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

    pub fn sizing_strategy(&self) -> SizingStrategy {
        self.shared.sizing_strategy.get().copied().unwrap_or_default()
    }

    pub fn run_until_closed(&self) -> Result<()> {
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

    pub fn show(&self) -> Result<()> {
        self.request(WindowThreadRequest::Show)
    }

    pub fn hide(&self) -> Result<()> {
        self.request(WindowThreadRequest::Hide)
    }

    pub fn is_open(&self) -> bool {
        !self.shared.stopped.load(Ordering::Relaxed)
    }

    pub fn is_resizable(&self) -> bool {
        self.shared.sizing_strategy().is_resizable()
    }

    pub fn min_size(&self) -> Option<Size> {
        self.shared.sizing_strategy().min_size()
    }

    pub fn max_size(&self) -> Option<Size> {
        self.shared.sizing_strategy().max_size()
    }

    pub fn handle_main_thread_callback(&self) {
        loop {
            let Some(receiver) = self.callback_receiver.as_ref() else { return };
            let Some(callback) = receiver.try_recv().ok() else { return };

            self.handle_main_thread_message(callback);
        }
    }

    pub fn set_parent(&self, new_parent: ParentWindowHandle) -> Result<()> {
        self.request(WindowThreadRequest::SetParent(new_parent))
    }

    pub fn request_poll(&self) -> Result<()> {
        self.shared.request_poll();
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
    ) -> Result<Self> {
        let mut ev_loop = calloop::EventLoop::try_new()?;
        let inner = WindowInner::create(options, &ev_loop, Arc::clone(&shared))?;

        shared.init(&inner);

        let handler = handler.build(WindowContext::new(Rc::clone(&inner)))?;
        let event_loop =
            EventLoop::new(inner, handler, receiver, sender, main_thread_caller, &mut ev_loop)?;

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
    pub fn receive(self) -> Result<LoopSignal> {
        let result = self.0.recv().map_err(|_| PlatformError::MainThreadRecvResult)?;

        match result {
            WindowOpenResult::Error(e) => Err(PlatformError::CreationFailed(e)),
            WindowOpenResult::Success { loop_signal } => Ok(loop_signal),
        }
    }
}
