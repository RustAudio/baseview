use crate::platform::x11::window_thread_channel::{thread_channel, HandleChannel, ThreadChannel};
use crate::WindowBuilder;
use calloop::EventLoop;
use std::error::Error;
use std::thread::{JoinHandle, Thread};

pub struct WindowThreadHandle {
    join_handle: JoinHandle<()>,
    channel: HandleChannel,
}

impl WindowThreadHandle {
    pub fn start(builder: WindowBuilder) -> Result<Self, Box<dyn Error>> {
        let (thread_channel, mut handle) = thread_channel();

        let join_handle = std::thread::spawn(move || window_thread(builder, thread_channel));

        let (wid, sig) = handle.wait_for_create();

        Ok(todo!())
    }
}

fn window_thread(builder: WindowBuilder, thread_channel: ThreadChannel) {
    // Create Event Loop
    // Create Window
    // Initialize handler

    // Send Created event
    // Add channel + X11 connection to event loop
    // Start loop
}
