use crate::ExamplePluginMainThread;
use crate::window_handler::OpenWindowExample;
use baseview::{WindowBuilder, WindowHandle};
use clack_extensions::gui::{
    AspectRatioStrategy, GuiApiType, GuiConfiguration, GuiResizeHints, GuiSize, PluginGuiImpl,
    Window,
};
use clack_plugin::plugin::PluginError;

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
        let handle = WindowBuilder::new().build(OpenWindowExample::new); // TODO: handle errors

        self.gui = Some(ExamplePluginGui { handle });

        Ok(())
    }

    fn destroy(&mut self) {
        let Some(gui) = self.gui.take() else { return };

        gui.handle.close()
    }

    fn set_scale(&mut self, scale: f64) -> Result<(), PluginError> {
        let Some(gui) = &self.gui else {
            return Err(PluginError::Message("Invalid GUI call: GUI is not created"));
        };

        gui.handle.suggest_fallback_scale(Some(scale));
        Ok(())
    }

    fn get_size(&mut self) -> Option<GuiSize> {
        let Some(gui) = &self.gui else {
            return None;
        };

        let uses_logical =
            matches!(GuiApiType::default_for_current_platform(), Some(a) if a.uses_logical_size());

        let size = gui.handle.size();

        if uses_logical {
            let size = size.logical.cast();
            Some(GuiSize { width: size.width, height: size.height })
        } else {
            Some(GuiSize { width: size.physical.width, height: size.physical.height })
        }
    }

    fn can_resize(&mut self) -> bool {
        true // Non-resizeable windows not supported yet
    }

    fn get_resize_hints(&mut self) -> Option<GuiResizeHints> {
        Some(GuiResizeHints {
            can_resize_horizontally: true,
            can_resize_vertically: true,
            strategy: AspectRatioStrategy::Disregard,
        })
    }

    fn adjust_size(&mut self, size: GuiSize) -> Option<GuiSize> {
        Some(size) // Not supported yet
    }

    fn set_size(&mut self, size: GuiSize) -> Result<(), PluginError> {
        todo!()
    }

    fn set_parent(&mut self, window: Window) -> Result<(), PluginError> {
        todo!()
    }

    fn set_transient(&mut self, window: Window) -> Result<(), PluginError> {
        todo!() // Not supported yet
    }

    fn suggest_title(&mut self, title: &str) {
        if let Some(gui) = &self.gui {
            gui.handle.set_title(title);
        }
    }

    fn show(&mut self) -> Result<(), PluginError> {
        todo!()
    }

    fn hide(&mut self) -> Result<(), PluginError> {
        todo!()
    }
}
