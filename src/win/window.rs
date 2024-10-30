use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::ptr::null_mut;
use std::rc::Rc;
use winapi::shared::windef::HWND;
use winapi::um::ole2::OleInitialize;

use raw_window_handle::{
    HasRawWindowHandle, RawDisplayHandle, RawWindowHandle, WindowsDisplayHandle,
};
use winapi::um::winuser::{LoadCursorW, SetCursor};

#[cfg(feature = "opengl")]
use crate::gl::win::GlContext;
use crate::win::cursor::cursor_to_lpcwstr;
use crate::win::handle::{WindowHandle, WindowHandleTransmitter};
use crate::win::proc::ProcState;
use crate::win::win32_window::Win32Window;
use crate::{MouseCursor, Size, WindowHandler, WindowOpenOptions};

/// Tasks that must be deferred until the end of [`wnd_proc()`] to avoid reentrant `WindowState`
/// borrows. See the docstring on [`Window::deferred_tasks`] for more information.
#[derive(Debug, Clone)]
enum WindowTask {
    /// Resize the window to the given size. The size is in logical pixels. DPI scaling is applied
    /// automatically.
    Resize(Size),
    Close,
}

pub struct Window {
    pub(crate) win32_window: Win32Window,
    cursor_icon: Cell<MouseCursor>,

    /// Tasks that should be executed at the end of `wnd_proc`.
    /// This is needed to avoid re-entrant calls into the `WindowHandler`.
    deferred_tasks: RefCell<VecDeque<WindowTask>>,
}

impl Window {
    pub fn open_parented<H, B>(
        parent: &impl HasRawWindowHandle, options: WindowOpenOptions, build: B,
    ) -> WindowHandle
    where
        H: WindowHandler + 'static,
        B: FnOnce(crate::Window) -> H,
        B: Send + 'static,
    {
        let parent = match parent.raw_window_handle() {
            RawWindowHandle::Win32(h) => h.hwnd as HWND,
            h => panic!("unsupported parent handle {:?}", h),
        };

        let window_handle = Self::open(Some(parent), options, build);

        window_handle
    }

    pub fn open_blocking<H: WindowHandler>(
        options: WindowOpenOptions, build_handler: impl FnOnce(crate::Window) -> H,
    ) {
        let handle = Self::open(None, options, build_handler);
        handle.block_on_window()
    }

    fn open<H: WindowHandler>(
        parent: Option<HWND>, options: WindowOpenOptions,
        build_handler: impl FnOnce(crate::Window) -> H,
    ) -> WindowHandle {
        // TODO: ?
        unsafe {
            OleInitialize(null_mut());
        }

        let win32_window = Win32Window::create(parent, &options);
        let window = Rc::new(Window {
            win32_window,
            cursor_icon: Cell::new(MouseCursor::Default),
            deferred_tasks: RefCell::new(VecDeque::with_capacity(4)),
        });
        let handler = build_handler(crate::Window::new(Rc::downgrade(&window)));

        let (tx, handle) = unsafe { WindowHandleTransmitter::new(window.win32_window.handle()) };

        ProcState::new(window, tx, handler).move_to_proc();

        handle
    }

    fn defer_task(&self, task: WindowTask) {
        self.deferred_tasks.borrow_mut().push_back(task)
    }

    pub fn close(&self) {
        self.defer_task(WindowTask::Close)
    }

    pub fn resize(&self, size: Size) {
        // To avoid reentrant event handler calls we'll defer the actual resizing until after the
        // event has been handled
        self.defer_task(WindowTask::Resize(size))
    }

    pub fn has_focus(&self) -> bool {
        self.win32_window.has_focus()
    }

    pub fn focus(&self) {
        self.win32_window.focus()
    }

    pub fn set_mouse_cursor(&self, mouse_cursor: MouseCursor) {
        self.cursor_icon.set(mouse_cursor);
        unsafe {
            let cursor = LoadCursorW(null_mut(), cursor_to_lpcwstr(mouse_cursor));
            SetCursor(cursor);
        }
    }

    #[cfg(feature = "opengl")]
    pub fn gl_context(&self) -> Option<std::rc::Weak<GlContext>> {
        self.win32_window.gl_context.as_ref().map(Rc::downgrade)
    }

    pub fn raw_window_handle(&self) -> RawWindowHandle {
        self.win32_window.raw_window_handle().into()
    }
    pub fn raw_display_handle(&self) -> RawDisplayHandle {
        WindowsDisplayHandle::empty().into()
    }

    pub(crate) fn handle_deferred_tasks(&self) {
        // NOTE: This is written like this instead of using a `for` loop to avoid extending
        //       the borrow of `window_state.deferred_tasks` into the call of
        //       `window_state.handle_deferred_task()` since that may also generate additional
        //       messages.
        while let Some(task) = self.pop_deferred_task() {
            self.handle_deferred_task(task);
        }
    }

    fn pop_deferred_task(&self) -> Option<WindowTask> {
        self.deferred_tasks.borrow_mut().pop_front()
    }

    /// Handle a deferred task as described in `Window::deferred_tasks`
    fn handle_deferred_task(&self, task: WindowTask) {
        match task {
            WindowTask::Resize(size) => self.win32_window.resize_logical(size),
            WindowTask::Close => self.win32_window.close(),
        }
    }
}

pub fn copy_to_clipboard(_data: &str) {
    todo!()
}
