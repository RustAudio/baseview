mod cursor;
mod dpi;
mod draw;
mod drop_target;
mod error;
#[cfg(feature = "opengl")]
pub mod gl;
mod hook;
mod keyboard;
mod mouse;
mod parent_handle;
mod platform_handle;
mod sizing;
mod waker;
mod window;
mod window_state;

pub(crate) use dpi::DpiScalingStrategy;
pub use error::{PlatformError, PlatformResult};
pub use parent_handle::*;
pub use platform_handle::*;
pub use waker::WindowWaker;
pub use window::*;

use crate::handler::WindowHandlerBuilder;
use crate::host::Host;
use crate::platform::win::drop_target::DropTarget;
use crate::platform::win::window_state::{WindowSharedState, WindowState};
use crate::wrappers::win32::{Rect, TimerId};
use crate::WindowHandler;
use ::dpi::{PhysicalSize, Size};
use std::cell::{Cell, OnceCell};
use std::rc::Rc;
use windows_core::ComObject;

pub type TimerHandle = TimerId;

pub type WindowContext = Rc<WindowState>;
pub type DamageRect = Rect;

pub fn assume_standalone_in_process() {
    dpi::set_process_dpi_awareness();
}

pub struct BaseviewWindow {
    window_state: Rc<WindowState>,
    shared_state: Rc<WindowSharedState>,
    initial_size: Size,

    handler_builder: Cell<Option<WindowHandlerBuilder>>,
    handler: OnceCell<Box<dyn WindowHandler>>,
    host: Host,

    // Workaround some VST3 hosts (e.g. Bitwig) not allowing request_resize before show() occurs
    pub host_needs_new_size_notified_on_show: Cell<Option<PhysicalSize<u32>>>,

    // Things not directly used, but kept so their Drop impl runs when the window is destroyed
    _keyboard_hook: Cell<Option<hook::KeyboardHookHandle>>,
    _drop_target: Cell<Option<ComObject<DropTarget>>>,

    #[cfg(feature = "opengl")]
    gl_config: Option<crate::gl::GlConfig>,
}
