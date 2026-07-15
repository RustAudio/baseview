use crate::window_handler::OpenWindowExample;
use crate::ExamplePluginMainThread;
use baseview::dpi::PhysicalSize;
use baseview::gl::GlConfig;
use baseview::{WindowHandle, WindowOpenOptions};
use clack_extensions::gui::{
    GuiApiType, GuiConfiguration, GuiResizeHints, GuiSize, PluginGuiImpl, Window as ClapWindow,
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

    fn set_scale(&mut self, _scale: f64) -> Result<(), PluginError> {
        // Unsupported
        Ok(())
    }

    fn get_size(&mut self) -> Option<GuiSize> {
        // Unsupported
        Some(GuiSize { width: 400, height: 200 })
    }

    fn can_resize(&mut self) -> bool {
        false // Non-resizeable windows not supported yet
    }

    fn get_resize_hints(&mut self) -> Option<GuiResizeHints> {
        None // Not supported yet
    }

    fn adjust_size(&mut self, _size: GuiSize) -> Option<GuiSize> {
        None // Not supported yet
    }

    fn set_size(&mut self, _size: GuiSize) -> Result<(), PluginError> {
        Ok(()) // Not supported yet
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
