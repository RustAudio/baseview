mod drop_target;
mod hook;
mod keyboard;
mod window;
mod window_state;

use std::rc::Rc;
pub use window::*;

#[cfg(feature = "opengl")]
pub mod gl;

pub type WindowContext = Rc<window_state::WindowState>;
