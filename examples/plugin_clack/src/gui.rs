use crate::window_handler::OpenWindowExample;
use crate::ExamplePluginMainThread;
use baseview::dpi::*;
use baseview::gl::GlConfig;
use baseview::{WindowHandle, WindowOpenOptions, WindowSize};
use clack_extensions::gui::{
    AspectRatioStrategy, GuiApiType, GuiConfiguration, GuiResizeHints, GuiSize, PluginGuiImpl,
    Window as ClapWindow,
};
use clack_plugin::plugin::PluginError;

#[allow(deprecated)]
use raw_window_handle::HasRawWindowHandle;

pub struct ExamplePluginGui {
    handle: WindowHandle,
}

impl PluginGuiImpl for ExamplePluginMainThread {
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
        // Delay creation until set_parent
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
        let Some(gui) = self.gui.as_ref() else {
            // Because we delayed the window creation, this will get called without a GUI active.
            // During that time, return the default UI size.
            return Some(GuiSize { width: 400, height: 200 });
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
        let parent = window.raw_window_handle()?;
        let parent = unsafe { raw_window_handle::WindowHandle::borrow_raw(parent) };

        let options = WindowOpenOptions::new()
            .with_size(PhysicalSize::new(400, 200))
            .with_gl_config(GlConfig::default())
            .with_parent(&parent);

        let window = baseview::create_window(options, OpenWindowExample::new)?;

        self.gui = Some(ExamplePluginGui { handle: window });

        Ok(())
    }

    fn set_transient(&mut self, _window: ClapWindow) -> Result<(), PluginError> {
        unimplemented!() // Not supported yet
    }

    fn suggest_title(&mut self, _title: &str) {
        // Not supported yet
    }

    fn show(&mut self) -> Result<(), PluginError> {
        Ok(()) // Not supported yet
    }

    fn hide(&mut self) -> Result<(), PluginError> {
        Ok(()) // Not supported yet
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
