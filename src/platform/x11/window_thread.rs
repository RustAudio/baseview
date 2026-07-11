use crate::handler::WindowHandlerBuilder;
use crate::platform::x11::event_loop::EventLoop;
use crate::platform::x11::window_thread_channel::{thread_channel, HandleChannel, ThreadChannel};
use crate::platform::{create_window_inner, X11Connection};
use crate::{WindowBuilder, WindowContext};
use calloop::LoopSignal;
use std::error::Error;
use std::num::NonZeroU32;
use std::rc::Rc;
use std::thread::JoinHandle;

pub struct WindowThreadHandle {
    join_handle: Option<JoinHandle<()>>,
    channel: HandleChannel,
    window_id: NonZeroU32,
    loop_signal: LoopSignal,
}

impl WindowThreadHandle {
    pub fn start(
        builder: WindowBuilder, handler: WindowHandlerBuilder,
    ) -> Result<Self, Box<dyn Error>> {
        let (thread_channel, mut handle_channel) = thread_channel();

        let join_handle =
            std::thread::spawn(move || window_thread(builder, handler, thread_channel));

        let (wid, sig) = handle_channel.wait_for_create();

        Ok(Self {
            join_handle: Some(join_handle),
            channel: handle_channel,
            window_id: wid,
            loop_signal: sig,
        })
    }

    pub fn join(mut self) {
        if let Some(handle) = self.join_handle.take() {
            handle.join().unwrap();
        }
    }
}

impl Drop for WindowThreadHandle {
    fn drop(&mut self) {
        self.loop_signal.stop();
        self.loop_signal.wakeup();

        if let Some(handle) = self.join_handle.take() {
            handle.join().unwrap();
        }
    }
}

fn window_thread(
    builder: WindowBuilder, handler: WindowHandlerBuilder, mut thread_channel: ThreadChannel,
) {
    // Connect to the X server
    let xcb_connection = Rc::new(X11Connection::new().unwrap());
    let inner = create_window_inner(builder, xcb_connection).unwrap();
    let handler = handler.build(WindowContext::new(Rc::clone(&inner)));

    let mut event_loop_inner = calloop::EventLoop::try_new().unwrap();

    thread_channel.send_create(inner.window_id, event_loop_inner.get_signal());

    let mut event_loop = EventLoop::new(inner, handler, &mut event_loop_inner);

    event_loop.run(event_loop_inner).unwrap();

    // Send Created event
    // Add channel + X11 connection to event loop
    // Start loop
}
