use crate::window_handler::OpenWindowExample;
use crate::ExamplePluginMainThread;
use baseview::dpi::*;
use baseview::gl::GlConfig;
use baseview::host::{Host, HostCallbacks, HostMainThreadCaller};
use baseview::{HandlerError, Window, WindowOpenOptions, WindowSize};
use clack_extensions::gui::{
    AspectRatioStrategy, GuiApiType, GuiConfiguration, GuiResizeHints, GuiSize, HostGui,
    PluginGuiImpl, Window as ClapWindow,
};
use clack_plugin::plugin::PluginError;
use clack_plugin::prelude::{HostMainThreadHandle, HostSharedHandle};
#[allow(deprecated)]
use raw_window_handle::HasRawWindowHandle;

pub struct ExamplePluginGui {
    pub handle: Window,
}

impl PluginGuiImpl for ExamplePluginMainThread<'_> {
    fn is_api_supported(&mut self, configuration: GuiConfiguration) -> bool {
        !configuration.is_floating
            && Some(configuration.api_type) == GuiApiType::default_for_current_platform()
    }

    fn get_preferred_api(&mut self) -> Option<GuiConfiguration<'_>> {
        Some(GuiConfiguration {
            api_type: GuiApiType::default_for_current_platform()?,
            is_floating: false,
        })
    }

    fn create(&mut self, _configuration: GuiConfiguration) -> Result<(), PluginError> {
        let options = WindowOpenOptions::new()
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

        let window = Window::create_with_host(options, OpenWindowExample::new, host)?;

        self.gui = Some(ExamplePluginGui { handle: window });
        Ok(())
    }

    fn destroy(&mut self) {
        let Some(gui) = self.gui.take() else { return };

        gui.handle.close()
    }

    fn set_scale(&mut self, scale: f64) -> Result<(), PluginError> {
        let Some(gui) = &self.gui else {
            return Err(PluginError::Message("set_scale called without a GUI active"));
        };
        gui.handle.suggest_fallback_scale_factor(scale)?;

        Ok(())
    }

    fn get_size(&mut self) -> Option<GuiSize> {
        let Some(gui) = &self.gui else {
            eprintln!("get_size called without a GUI active");
            return None;
        };

        Some(window_size_to_gui_size(gui.handle.size()))
    }

    fn can_resize(&mut self) -> bool {
        true // Non-resizeable windows not supported yet
    }

    fn get_resize_hints(&mut self) -> Option<GuiResizeHints> {
        Some(GuiResizeHints {
            strategy: AspectRatioStrategy::Disregard, // Not supported

            // Non-resizeable windows not supported yet
            can_resize_vertically: true,
            can_resize_horizontally: true,
        })
    }

    fn adjust_size(&mut self, size: GuiSize) -> Option<GuiSize> {
        Some(size) // Not supported yet
    }

    fn set_size(&mut self, size: GuiSize) -> Result<(), PluginError> {
        let Some(gui) = &self.gui else {
            return Err(PluginError::Message("set_size called without a GUI active"));
        };

        let size = gui_size_to_window_size(size);
        gui.handle.resize(size)?;

        Ok(())
    }

    #[allow(deprecated)]
    fn set_parent(&mut self, window: ClapWindow) -> Result<(), PluginError> {
        let Some(gui) = &self.gui else {
            return Err(PluginError::Message("set_parent called without a GUI active"));
        };

        let parent = window.raw_window_handle()?;
        let parent = unsafe { raw_window_handle::WindowHandle::borrow_raw(parent) };

        gui.handle.set_parent(&parent)?;
        gui.handle.show()?;

        Ok(())
    }

    fn set_transient(&mut self, _window: ClapWindow) -> Result<(), PluginError> {
        unimplemented!() // Not supported yet
    }

    fn suggest_title(&mut self, _title: &str) {
        // Not supported yet
    }

    fn show(&mut self) -> Result<(), PluginError> {
        let Some(gui) = &self.gui else {
            return Err(PluginError::Message("show called without a GUI active"));
        };
        gui.handle.show()?;

        Ok(())
    }

    fn hide(&mut self) -> Result<(), PluginError> {
        let Some(gui) = &self.gui else {
            return Err(PluginError::Message("hide called without a GUI active"));
        };
        gui.handle.show()?;

        Ok(())
    }
}

fn window_size_to_gui_size(size: WindowSize) -> GuiSize {
    #[cfg(target_os = "macos")]
    {
        let size = size.logical.cast();
        GuiSize { width: size.width, height: size.height }
    }

    #[cfg(not(target_os = "macos"))]
    {
        let size = size.physical.cast();
        GuiSize { width: size.width, height: size.height }
    }
}

fn gui_size_to_window_size(size: GuiSize) -> Size {
    #[cfg(target_os = "macos")]
    {
        Size::Logical(LogicalSize::new(size.width, size.height).cast())
    }
    #[cfg(not(target_os = "macos"))]
    {
        Size::Physical(PhysicalSize::new(size.width, size.height))
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
        let size = window_size_to_gui_size(new_size);
        self.ext.request_resize(&self.host, size.width, size.height)?;
        Ok(())
    }

    fn destroyed(&mut self) {
        self.ext.closed(&self.host, true);
    }
}
