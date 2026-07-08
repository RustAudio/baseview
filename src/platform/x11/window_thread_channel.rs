use calloop::LoopSignal;
use std::num::NonZeroU32;
use std::sync::mpsc;
use std::sync::mpsc::Receiver;

enum ThreadToHandleMessage {
    WindowCreated { window_id: NonZeroU32, loop_signal: LoopSignal },
}

enum HandleToThreadMessage {}

pub struct ThreadChannel {
    pub send: mpsc::Sender<ThreadToHandleMessage>,
    pub recv: calloop::channel::Channel<HandleToThreadMessage>,
}

impl ThreadChannel {
    pub fn send_create(&mut self, window_id: NonZeroU32, loop_signal: LoopSignal) {
        self.send.send(ThreadToHandleMessage::WindowCreated { window_id, loop_signal }).unwrap();
    }
}

pub struct HandleChannel {
    pub recv: Receiver<ThreadToHandleMessage>,
    pub send: calloop::channel::Sender<HandleToThreadMessage>,
}

impl HandleChannel {
    pub fn wait_for_create(&mut self) -> (NonZeroU32, LoopSignal) {
        loop {
            let msg = self.recv.recv().unwrap();
            if let ThreadToHandleMessage::WindowCreated { window_id, loop_signal } = msg {
                return (window_id, loop_signal);
            }
        }
    }
}

pub fn thread_channel() -> (ThreadChannel, HandleChannel) {
    let (st, rt) = calloop::channel::channel();
    let (send, recv) = mpsc::channel::<ThreadToHandleMessage>();

    (ThreadChannel { send, recv: rt }, HandleChannel { recv, send: st })
}
