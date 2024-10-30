use crate::win::util::to_wstr;
use crate::{PhySize, Size, WindowInfo, WindowOpenOptions, WindowScalePolicy};
use raw_window_handle::Win32WindowHandle;
use std::cell::Cell;
use std::ffi::c_void;
use std::ptr::null_mut;
use winapi::shared::minwindef::{DWORD, UINT};
use winapi::shared::windef::{HWND, RECT};
use winapi::um::winuser::{
    AdjustWindowRectEx, CreateWindowExW, GetDpiForWindow, GetFocus, KillTimer, PostMessageW,
    SetFocus, SetProcessDpiAwarenessContext, SetTimer, SetWindowPos, SWP_NOMOVE, SWP_NOZORDER,
    WM_USER, WS_CAPTION, WS_CHILD, WS_CLIPSIBLINGS, WS_MAXIMIZEBOX, WS_MINIMIZEBOX, WS_POPUPWINDOW,
    WS_SIZEBOX, WS_VISIBLE,
};

mod class;
use class::*;

pub(crate) struct Win32Window {
    _class: WndClass,
    handle: HWND,
    style_flags: DWORD,

    current_size: Cell<WindowInfo>,
    scale_policy: WindowScalePolicy,

    frame_timer_started: Cell<bool>,

    #[cfg(feature = "opengl")]
    pub(crate) gl_context: Option<std::rc::Rc<crate::gl::win::GlContext>>,
}

impl Win32Window {
    // TODO: manage errors
    pub fn create(parent: Option<HWND>, options: &WindowOpenOptions) -> Self {
        // FIXME: try not to re-register a new class on every window open
        let class = WndClass::register();

        let initial_scaling = match options.scale {
            WindowScalePolicy::SystemScaleFactor => 1.0,
            WindowScalePolicy::ScaleFactor(scale) => scale,
        };

        let initial_size = WindowInfo::from_logical_size(options.size, initial_scaling);

        let parented = parent.is_some();

        let style_flags = if parented {
            WS_CHILD | WS_VISIBLE
        } else {
            WS_POPUPWINDOW
                | WS_CAPTION
                | WS_VISIBLE
                | WS_SIZEBOX
                | WS_MINIMIZEBOX
                | WS_MAXIMIZEBOX
                | WS_CLIPSIBLINGS
        };

        let window_size = if parented {
            initial_size.physical_size()
        } else {
            client_size_to_window_size(initial_size.physical_size(), style_flags)
        };

        let title = to_wstr(&options.title);
        let handle = unsafe {
            CreateWindowExW(
                0,
                class.atom() as _,
                title.as_ptr(),
                style_flags,
                0, // TODO: initial position
                0,
                window_size.width as i32,
                window_size.height as i32,
                parent.unwrap_or(null_mut()) as *mut _,
                null_mut(),
                null_mut(),
                null_mut(),
            )
        };

        // TODO: GL context
        let mut window = Self {
            _class: class,
            handle,
            style_flags,
            current_size: Cell::new(initial_size),
            scale_policy: options.scale,
            frame_timer_started: Cell::new(false),
            #[cfg(feature = "opengl")]
            gl_context: None,
        };

        // FIXME: this should NOT be changed if the window is part of an host
        // Only works on Windows 10.
        unsafe {
            SetProcessDpiAwarenessContext(
                winapi::shared::windef::DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE,
            );
        }

        // Now we can get the actual dpi of the window.
        window.check_for_dpi_changes();

        #[cfg(feature = "opengl")]
        window.create_gl_context(options);
        window.start_frame_timer();

        window
    }

    fn current_system_scale_factor(&self) -> f64 {
        // FIXME: Only works on Windows 10.
        let dpi = unsafe { GetDpiForWindow(self.handle) };
        dpi as f64 / 96.0
    }

    pub fn raw_window_handle(&self) -> Win32WindowHandle {
        let mut handle = Win32WindowHandle::empty();
        handle.hwnd = self.handle() as *mut c_void;
        handle
    }

    #[cfg(feature = "opengl")]
    fn create_gl_context(&mut self, options: &WindowOpenOptions) {
        self.gl_context = options.gl_config.as_ref().map(|gl_config| {
            let ctx =
                // SAFETY: TODO
                unsafe { crate::gl::win::GlContext::create(&self.raw_window_handle(), gl_config.clone()) }
                    .expect("Could not create OpenGL context");
            std::rc::Rc::new(ctx)
        });
    }

    fn resize(&self, size: PhySize) {
        let window_size = client_size_to_window_size(size, self.style_flags);

        // Windows makes us resize the window manually. This will trigger another `WM_SIZE` event,
        // which we can then send the user the new scale factor.
        unsafe {
            SetWindowPos(
                self.handle,
                self.handle,
                0,
                0,
                window_size.width as i32,
                window_size.height as i32,
                SWP_NOZORDER | SWP_NOMOVE,
            );
        }
    }

    fn check_for_dpi_changes(&self) {
        // Do not do anything if the scale factor is forced by the user
        if self.scale_policy != WindowScalePolicy::SystemScaleFactor {
            return;
        }
        let current_size = self.current_size.get();

        let current_scale_factor = self.current_system_scale_factor();
        if current_scale_factor == current_size.scale() {
            return;
        }

        let new_size =
            WindowInfo::from_logical_size(current_size.logical_size(), current_scale_factor);
        self.resize(new_size.physical_size());
        self.current_size.set(new_size);
    }

    pub fn has_focus(&self) -> bool {
        let focused_window = unsafe { GetFocus() };
        focused_window == self.handle
    }

    pub fn focus(&self) {
        unsafe {
            SetFocus(self.handle);
        }
    }

    pub fn handle(&self) -> HWND {
        self.handle
    }

    pub fn resize_logical(&self, size: Size) {
        let current_size = self.current_size.get();
        // TODO: use updated current scale instead?
        let new_size = WindowInfo::from_logical_size(size, current_size.scale());
        self.resize(new_size.physical_size())
    }

    /// Called when the size has been changed from an external event.
    /// Returns None if the size didn't actually change.
    pub fn resized(&self, new_size: PhySize) -> Option<WindowInfo> {
        let current_size = self.current_size.get();

        if current_size.physical_size() == new_size {
            return None;
        }

        let new_size = WindowInfo::from_physical_size(new_size, current_size.scale());
        self.current_size.set(new_size);

        Some(new_size)
    }

    pub fn update_scale_factor(&self, new_scale_factor: f64) {
        if self.scale_policy != WindowScalePolicy::SystemScaleFactor {
            // We don't care about DPI updates if the scale factor is forced by the user.
            return;
        }

        let current_size = self.current_size.get();
        let new_size = WindowInfo::from_logical_size(current_size.logical_size(), new_scale_factor);
        self.resize(new_size.physical_size());
        self.current_size.set(new_size);
    }

    pub fn current_size(&self) -> WindowInfo {
        self.current_size.get()
    }

    pub const WIN_FRAME_TIMER: usize = 4242;
    pub fn start_frame_timer(&self) {
        if self.frame_timer_started.get() {
            return;
        }

        unsafe { SetTimer(self.handle, Self::WIN_FRAME_TIMER, 15, None) };

        self.frame_timer_started.set(true)
    }

    pub fn stop_frame_timer(&self) {
        if !self.frame_timer_started.get() {
            return;
        }

        unsafe { KillTimer(self.handle, Self::WIN_FRAME_TIMER) };
        self.frame_timer_started.set(false)
    }

    pub const BV_WINDOW_MUST_CLOSE: UINT = WM_USER + 1;

    pub unsafe fn request_close(handle: HWND) {
        PostMessageW(handle, Self::BV_WINDOW_MUST_CLOSE, 0, 0);
    }

    pub fn close(&self) {
        unsafe { Self::request_close(self.handle) }
    }
}

impl Drop for Win32Window {
    fn drop(&mut self) {
        self.stop_frame_timer()
    }
}

pub fn client_size_to_window_size(size: PhySize, window_flags: DWORD) -> PhySize {
    let mut rect = RECT {
        left: 0,
        top: 0,
        // todo: check if usize fits into i32
        right: size.width as i32,
        bottom: size.height as i32,
    };

    unsafe {
        AdjustWindowRectEx(&mut rect, window_flags, 0, 0);
    }

    // TODO: saturating operations?
    PhySize { width: (rect.right - rect.left) as u32, height: (rect.bottom - rect.top) as u32 }
}
