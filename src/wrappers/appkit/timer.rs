use block2::RcBlock;
use objc2::rc::Weak;
use objc2_core_foundation::{
    kCFAllocatorDefault, kCFRunLoopDefaultMode, CFRetained, CFRunLoop, CFRunLoopTimer,
    CFTimeInterval,
};

pub struct TimerHandle {
    run_loop: Weak<CFRunLoop>,
    timer: CFRetained<CFRunLoopTimer>,
}

impl TimerHandle {
    pub fn new(interval: CFTimeInterval, closure: impl Fn() + 'static) -> Option<Self> {
        let run_loop = CFRunLoop::current()?;

        let block = RcBlock::new(move |_| closure());

        let allocator = unsafe { kCFAllocatorDefault };
        let timer =
            unsafe { CFRunLoopTimer::with_handler(allocator, 0.0, interval, 0, 0, Some(&block)) }?;

        let loop_mode = unsafe { kCFRunLoopDefaultMode };
        run_loop.add_timer(Some(&timer), loop_mode);

        Some(Self { run_loop: Weak::from_retained(&run_loop.into()), timer })
    }
}

impl Drop for TimerHandle {
    fn drop(&mut self) {
        let Some(run_loop) = self.run_loop.load() else {
            return;
        };

        let loop_mode = unsafe { kCFRunLoopDefaultMode };

        run_loop.remove_timer(Some(&self.timer), loop_mode);
    }
}
