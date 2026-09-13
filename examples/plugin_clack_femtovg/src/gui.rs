use crate::window_handler::FemtovgExample;
use crate::ExamplePluginMainThread;
use baseview::dpi::*;
use baseview::gl::GlConfig;
use baseview::host::{Host, HostCallbacks, HostMainThreadCaller};
use baseview::{HandlerError, Window, WindowSettings, WindowSize};
use clack_extensions::gui::{
    AspectRatioStrategy, GuiApiType, GuiConfiguration, GuiResizeHints, GuiSize, HostGui,
    PluginGuiImpl, Window as ClapWindow,
};
use clack_plugin::plugin::PluginError;
use clack_plugin::prelude::{HostMainThreadHandle, HostSharedHandle};

pub struct ExamplePluginGui {
    pub handle: Window,
}

impl PluginGuiImpl for ExamplePluginMainThread<'_> {
    fn is_api_supported(&self, configuration: GuiConfiguration) -> bool {
        !configuration.is_floating
            && Some(configuration.api_type) == GuiApiType::default_for_current_platform()
    }

    fn get_preferred_api(&self) -> Option<GuiConfiguration<'_>> {
        Some(GuiConfiguration {
            api_type: GuiApiType::default_for_current_platform()?,
            is_floating: false,
        })
    }

    fn create(&self, _configuration: GuiConfiguration) -> Result<(), PluginError> {
        let options = WindowSettings::new()
            .wait_for_parent()
            .with_size(PhysicalSize::new(400, 200))
            .with_gl_config(GlConfig::default());

        let mut host = Host::new().with_main_thread(unsafe {
            MainThreadHandler { host: self.host.shared().with_arbitrary_lifetime() }
        });

        if let Some(gui) = self.host_gui {
            host = host.with_callbacks(unsafe {
                HostGuiCallbacks { ext: gui, host: self.host.with_arbitrary_lifetime() }
            });
        }

        let window = Window::create_with_host(options, FemtovgExample::new, host)?;

        self.gui.replace(Some(ExamplePluginGui { handle: window }));
        Ok(())
    }

    fn destroy(&self) {
        let Some(gui) = self.gui.take() else { return };

        gui.handle.close()
    }

    fn set_scale(&self, scale: f64) -> Result<(), PluginError> {
        let Some(gui) = self.borrow_window() else {
            return Err(PluginError::Message("set_scale called without a GUI active"));
        };
        gui.suggest_fallback_scale_factor(scale)?;

        Ok(())
    }

    fn get_size(&self) -> Option<GuiSize> {
        let Some(gui) = self.borrow_window() else {
            eprintln!("get_size called without a GUI active");
            return None;
        };

        let size = gui.size().to_native_size();

        Some(GuiSize { width: size.width, height: size.height })
    }

    fn can_resize(&self) -> bool {
        let Some(gui) = self.borrow_window() else { return false };

        gui.is_resizable()
    }

    fn get_resize_hints(&self) -> Option<GuiResizeHints> {
        let can_resize = self.can_resize();

        Some(GuiResizeHints {
            strategy: AspectRatioStrategy::Disregard, // Not supported

            can_resize_vertically: can_resize,
            can_resize_horizontally: can_resize,
        })
    }

    fn adjust_size(&self, size: GuiSize) -> Option<GuiSize> {
        let Some(gui) = self.borrow_window() else { return None };

        let size = gui.adjust_size(NativeSize::new(size.width, size.height));

        Some(GuiSize { width: size.width, height: size.height })
    }

    fn set_size(&self, size: GuiSize) -> Result<(), PluginError> {
        let Some(gui) = self.borrow_window() else {
            return Err(PluginError::Message("set_size called without a GUI active"));
        };

        gui.resize(NativeSize { width: size.width, height: size.height })?;

        Ok(())
    }

    fn set_parent(&self, window: ClapWindow) -> Result<(), PluginError> {
        let Some(gui) = self.borrow_window() else {
            return Err(PluginError::Message("set_parent called without a GUI active"));
        };

        // SAFETY: The CLAP spec ensures the parent window handle is valid for at least this call
        let parent = unsafe { window.borrow_handle_unchecked()? };

        gui.set_parent(&parent)?;
        gui.show()?;

        Ok(())
    }

    fn set_transient(&self, _window: ClapWindow) -> Result<(), PluginError> {
        unimplemented!() // Not supported yet
    }

    fn suggest_title(&self, _title: &str) {
        // Not supported yet
    }

    fn show(&self) -> Result<(), PluginError> {
        let Some(gui) = self.borrow_window() else {
            return Err(PluginError::Message("show called without a GUI active"));
        };
        gui.show()?;

        Ok(())
    }

    fn hide(&self) -> Result<(), PluginError> {
        let Some(gui) = self.borrow_window() else {
            return Err(PluginError::Message("hide called without a GUI active"));
        };
        gui.show()?;

        Ok(())
    }
}

struct MainThreadHandler {
    host: HostSharedHandle<'static>,
}

impl HostMainThreadCaller for MainThreadHandler {
    fn call_main_thread(&mut self) {
        self.host.request_callback();
    }
}

struct HostGuiCallbacks {
    ext: HostGui,
    host: HostMainThreadHandle<'static>,
}

impl HostCallbacks for HostGuiCallbacks {
    fn request_resize(&mut self, new_size: WindowSize) -> Result<(), HandlerError> {
        let new_size = new_size.to_native_size();
        self.ext.request_resize(&self.host, new_size.width, new_size.height)?;
        Ok(())
    }

    fn destroyed(&mut self) {
        self.ext.closed(&self.host, true);
    }
}
