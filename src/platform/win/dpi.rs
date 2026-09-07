use crate::wrappers::win32::window::HWnd;
use crate::wrappers::win32::{
    DpiAwarenessContext, DpiAwarenessContextType, ExtendedShCore, ExtendedUser32, LibraryModule,
    ProcessDpiAwareness,
};
use crate::WindowSettings;

pub(crate) enum StrategyType {
    Assume96Dpi,
}

#[derive(Copy, Clone, Default)]
pub(crate) struct DpiScalingStrategy {
    assume_96_dpi: bool,
    thread_dpi_awareness_context_type: Option<DpiAwarenessContextType>,
}

impl DpiScalingStrategy {
    pub fn get(user32: &ExtendedUser32, parent: Option<HWnd>, settings: &WindowSettings) -> Self {
        // If we have an OpenGL context, then we must follow the process' DPI awareness setting.
        //
        // We hope it's the same as the one for the (potential) parent window, or that it's at least
        // compatible. But if not, then having a DPI Awareness reset due to mismatched DPI Awareness
        // is still better than half of the window being unusuable.
        #[cfg(feature = "opengl")]
        if settings.gl_config.is_some() {
            return Self::get_from_process();
        }

        if let Some(parent) = parent {
            let parent_context = parent.get_dpi_awareness_context(user32);

            if parent.supports_mixed_dpi_hosting_behavior(user32) {
                todo!()
            }

            // Check if parent supports mixed context
            // If not, use parent DPI awareness

            // If it does, choose whatever suits us best!
            todo!()
        } else {
            // No parent, we can choose whatever suits us best!

            Self::get_best_supported()
        }
    }

    // If we have an OpenGL context, we *must* follow the process
    // DPI awareness, otherwise some OpenGL Drivers (namely AMD and NVIDIA) will crap out.
    // See: https://github.com/RustAudio/baseview/issues/321
    //      https://forum.juce.com/t/scaling-issues-with-retina-and-opengl-on-windows-in-ableton-live/41573
    //      https://forums.developer.nvidia.com/t/high-dpi-scaling-bug-with-nvidia-drivers-and-opengl-on-windows-10/79615
    #[cfg(feature = "opengl")]
    fn get_from_process() -> Self {
        todo!()
    }

    fn get_best_supported() -> Self {
        todo!()
    }

    fn get_from_parent() -> Self {
        todo!()
    }

    pub fn assume_96_dpi(&self) -> bool {
        todo!()
    }

    pub fn thread_dpi_awareness_context_type(&self) -> Option<DpiAwarenessContextType> {
        todo!()
    }
}

pub(crate) fn set_process_dpi_awareness() {
    if !set_process_dpi_awareness_context() {
        // Win8.1 fallback
        set_process_dpi_awareness_legacy()
    }
}

fn set_process_dpi_awareness_context() -> bool {
    let user32 = match LibraryModule::<ExtendedUser32>::load() {
        Ok(user32) => user32,
        Err(e) => {
            crate::warn!("Failed to load user32.dll: {}", e);
            return false;
        }
    };

    let Some(supported) = DpiAwarenessContextType::best_supported(&user32) else { return false };

    match DpiAwarenessContext::from(supported).set_process(&user32) {
        None => false,
        Some(Ok(())) => true,
        Some(Err(e)) => {
            crate::warn!("Failed to set Process DPI Awareness Context: {}", e);
            false
        }
    }
}

fn set_process_dpi_awareness_legacy() {
    let shcore = match LibraryModule::<ExtendedShCore>::load() {
        Ok(user32) => user32,
        Err(e) => {
            crate::warn!("Failed to load api-ms-win-shcore-scaling-l1-1-1.dll: {}", e);
            return;
        }
    };

    if let Err(e) = ProcessDpiAwareness::PerMonitorDpiAware.set(&shcore) {
        crate::warn!("Failed to set Process DPI Awareness: {}", e);
    }
}
