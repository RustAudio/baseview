use crate::audio::ExamplePluginAudioProcessor;
use crate::gui::ExamplePluginGui;
use clack_extensions::gui::{HostGui, PluginGui};
use clack_plugin::prelude::*;

mod audio;
mod gui;
mod window_handler;

/// The type that represents our plugin in Clack.
///
/// This is what implements the [`Plugin`] trait, where all the other subtypes are attached.
pub struct ExamplePlugin;

impl Plugin for ExamplePlugin {
    type AudioProcessor<'a> = ExamplePluginAudioProcessor;
    type Shared<'a> = ();
    type MainThread<'a> = ExamplePluginMainThread<'a>;

    fn declare_extensions(builder: &mut PluginExtensions<Self>, _shared: Option<&()>) {
        builder.register::<PluginGui>();
    }
}

impl DefaultPluginFactory for ExamplePlugin {
    fn get_descriptor() -> PluginDescriptor {
        use clack_plugin::plugin::features::*;

        PluginDescriptor::new("org.rust-audio.clack.gain-baseview", "Clack Gain Baseview Example")
            .with_features([AUDIO_EFFECT, STEREO])
    }

    fn new_shared(_host: HostSharedHandle<'_>) -> Result<Self::Shared<'_>, PluginError> {
        Ok(())
    }

    fn new_main_thread<'a>(
        host: HostMainThreadHandle<'a>, _shared: &'a Self::Shared<'a>,
    ) -> Result<Self::MainThread<'a>, PluginError> {
        Ok(Self::MainThread { gui: None, host_gui: host.get_extension(), host })
    }
}

/// The data that belongs to the main thread of our plugin.
pub struct ExamplePluginMainThread<'a> {
    /// The host handle
    host: HostMainThreadHandle<'a>,
    // The host GUI extension handle
    host_gui: Option<HostGui>,
    /// The plugin's GUI state and context
    gui: Option<ExamplePluginGui>,
}

impl<'a> PluginMainThread<'a, ()> for ExamplePluginMainThread<'a> {
    fn on_main_thread(&mut self) {
        if let Some(gui) = self.gui.as_mut() {
            gui.handle.host_main_thread_callback();
        }
    }
}

clack_export_entry!(SinglePluginEntry<ExamplePlugin>);
