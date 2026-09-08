use crate::wrappers::win32::window::HWnd;
use crate::wrappers::win32::{
    Dpi, DpiAwarenessContext, DpiAwarenessContextType, DpiAwarenessGuard, ExtendedShCore,
    ExtendedUser32, LazyLibraryModule, LibraryModule, ProcessDpiAwareness,
};
use crate::WindowSettings;
use std::cell::LazyCell;
use std::ops::Deref;

#[derive(Copy, Clone, Default)]
pub(crate) struct DpiScalingStrategy {
    pub assume_96_dpi: bool,
    pub thread_dpi_awareness_context: Option<DpiAwarenessContext>,
}

impl DpiScalingStrategy {
    pub fn get(
        user32: Option<&ExtendedUser32>, parent: Option<HWnd>, settings: &WindowSettings,
    ) -> Self {
        let _span = crate::debug_span!("DpiScalingStrategy");
        let shcore = LibraryModule::<ExtendedShCore>::lazy();

        // If we have an OpenGL context, then we must follow the process' DPI awareness setting.
        // Otherwise, some OpenGL Drivers (namely AMD and NVIDIA) will crap out.
        //
        // We hope it's the same as the one for the (potential) parent window, or that it's at least
        // compatible. But if not, then having a DPI Awareness reset due to mismatched DPI Awareness
        // is still better than half of the window being unusable.
        //
        // See: https://github.com/RustAudio/baseview/issues/321
        //      https://forum.juce.com/t/scaling-issues-with-retina-and-opengl-on-windows-in-ableton-live/41573
        //      https://forums.developer.nvidia.com/t/high-dpi-scaling-bug-with-nvidia-drivers-and-opengl-on-windows-10/79615
        #[cfg(feature = "opengl")]
        if settings.gl_config.is_some() {
            crate::debug!("OpenGL context requested: bypassing DPI awareness checking, using process DPI awareness instead.");
            return Self::get_from_process(user32, &shcore);
        }

        let Some(user32_lib) = user32 else {
            crate::debug!("user32.dll is unavailable: falling back to legacy Windows 8 APIs for DPI awareness detection.");
            return Self::get_from_process_legacy(&shcore);
        };

        if let Some(parent) = parent {
            let Some(parent_dpi_ctx) = parent.get_dpi_awareness_context(user32_lib) else {
                crate::debug!("Could not get DPI Awareness Context from parent, falling back to process DPI Awareness.");
                return Self::get_from_process(user32, &shcore);
            };

            if parent.supports_mixed_dpi_hosting_behavior(user32_lib) {
                Self::get_best_matching_with_dpi_parent_awareness_context(
                    parent_dpi_ctx,
                    user32_lib,
                    &shcore,
                )
            } else {
                Self::get_from_specific_dpi_awareness_context(parent_dpi_ctx, user32_lib)
            }
        } else {
            // No parent, we can choose whatever suits us best!
            Self::get_best_supported(user32_lib, &shcore)
        }
    }

    fn get_from_process(
        user32: Option<&ExtendedUser32>, shcore: &LazyLibraryModule<ExtendedShCore>,
    ) -> Self {
        let Some(user32) = user32 else {
            return Self::get_from_process_legacy(shcore);
        };

        let Some(dpi_awareness_context) = DpiAwarenessContext::get_from_process(user32) else {
            return Self::get_from_process_legacy(shcore);
        };

        Self::get_from_specific_dpi_awareness_context(dpi_awareness_context, user32)
    }

    fn get_best_matching_with_dpi_parent_awareness_context(
        parent_dpi_awareness_context: DpiAwarenessContext, user32: &ExtendedUser32,
        shcore: &LazyLibraryModule<ExtendedShCore>,
    ) -> Self {
        use DpiAwarenessContextType::*;

        let dpi_awareness_type = parent_dpi_awareness_context.get_type(user32);
        crate::debug!(
            "Parent DPI hosting behavior is mixed, parent DPI Awareness Context type is {:?}.",
            dpi_awareness_type
        );

        // These are documented to not be compatible with per-monitor awareness types, so we'll fall back to System-aware
        // See: https://learn.microsoft.com/en-us/windows/win32/api/windef/ne-windef-dpi_hosting_behavior#remarks
        if matches!(dpi_awareness_type, Some(Unaware | UnawareGDIScaled | SystemDpiAware)) {
            crate::debug!("Parent has DPI Awareness Context with Per-Monitor DPI awareness, falling back to System DPI Awareness.");
            return Self {
                assume_96_dpi: false,
                thread_dpi_awareness_context: Some(SystemDpiAware.into()),
            };
        }

        Self::get_best_supported(user32, shcore)
    }

    fn get_from_specific_dpi_awareness_context(
        dpi_awareness_context: DpiAwarenessContext, user32: &ExtendedUser32,
    ) -> Self {
        use DpiAwarenessContextType::*;

        let dpi_awareness_type = dpi_awareness_context.get_type(user32);

        crate::debug!("Using DPI Awareness Context of type {:?}.", dpi_awareness_type);

        // If type is unknown, assume it's better than System-Aware, and we can at least fetch the actual DPI.
        let assume_96_dpi = matches!(dpi_awareness_type, Some(Unaware | UnawareGDIScaled));

        Self { assume_96_dpi, thread_dpi_awareness_context: Some(dpi_awareness_context) }
    }

    fn get_from_process_legacy(shcore: &LazyLibraryModule<ExtendedShCore>) -> Self {
        use ProcessDpiAwareness::*;

        let Some(shcore) = LazyCell::deref(shcore) else { return Self::completely_unaware() };

        let awareness = ProcessDpiAwareness::get(shcore);

        crate::debug!("Using legacy Process DPI Awareness: {:?}", awareness);

        let assume_96_dpi = matches!(awareness, None | Some(Unaware));

        Self { assume_96_dpi, thread_dpi_awareness_context: None }
    }

    fn completely_unaware() -> Self {
        Self { assume_96_dpi: true, thread_dpi_awareness_context: None }
    }

    fn get_best_supported(
        user32: &ExtendedUser32, shcore: &LazyLibraryModule<ExtendedShCore>,
    ) -> Self {
        let Some(best_supported) = DpiAwarenessContextType::best_supported(user32) else {
            crate::debug!("No DPI Awareness Context types are available. Falling back to legacy Windows 8 APIs.");
            return Self::get_from_process_legacy(shcore);
        };

        Self::get_from_specific_dpi_awareness_context(best_supported.into(), user32)
    }
}

impl DpiScalingStrategy {
    pub fn get_dpi_for_window(&self, own_window: HWnd, user32: &ExtendedUser32) -> Option<Dpi> {
        if self.assume_96_dpi {
            return Some(Dpi::default());
        }

        DpiAwarenessGuard::with_guard_optional(user32, *self, || {
            if let Some(dpi) = own_window.get_dpi(user32) {
                return Some(dpi);
            }

            if let Some(dpi) =
                own_window.get_dpi_awareness_context(user32).and_then(|d| d.dpi(user32))
            {
                return Some(dpi);
            }

            let shcore = LibraryModule::<ExtendedShCore>::lazy();

            if let Some(dpi) =
                shcore.as_ref().and_then(|shcore| own_window.get_dpi_from_monitor(shcore))
            {
                return Some(dpi);
            }

            Dpi::get_system(user32)
        })
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
