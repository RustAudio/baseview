use crate::wrappers::win32::window::HWnd;
use crate::wrappers::win32::ExtendedUser32;

pub enum StrategyType {
    Assume96Dpi,
}

#[derive(Copy, Clone, Default)]
pub struct DpiScalingStrategy {}

impl DpiScalingStrategy {
    pub fn get(user32: &ExtendedUser32, parent: Option<HWnd>) -> Self {
        if let Some(parent) = parent {
            todo!()
        } else {
            todo!()
        }
    }

    pub fn assume_96_dpi(&self) -> bool {
        todo!()
    }
}
