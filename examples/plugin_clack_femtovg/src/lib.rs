use crate::audio::ExamplePluginAudioProcessor;
use crate::gui::ExamplePluginGui;
use clack_extensions::gui::{HostGui, PluginGui};
use clack_extensions::state::{PluginState, PluginStateImpl};
use clack_plugin::prelude::*;
use clack_plugin::stream::{InputStream, OutputStream};
use std::cell::{Ref, RefCell};

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
        builder.register::<PluginGui>().register::<PluginState>();
    }
}

impl DefaultPluginFactory for ExamplePlugin {
    fn get_descriptor() -> PluginDescriptor {
        use clack_plugin::plugin::features::*;

        PluginDescriptor::new(
            "org.rust-audio.clack.gain-baseview-femtovg",
            "Clack Gain Baseview Femtovg Example",
        )
        .with_features([AUDIO_EFFECT, STEREO])
    }

    fn new_shared(_host: HostSharedHandle<'_>) -> Result<Self::Shared<'_>, PluginError> {
        Ok(())
    }

    fn new_main_thread<'a>(
        host: HostMainThreadHandle<'a>, _shared: &'a Self::Shared<'a>,
    ) -> Result<Self::MainThread<'a>, PluginError> {
        Ok(Self::MainThread { gui: None.into(), host_gui: host.get_extension(), host })
    }
}

/// The data that belongs to the main thread of our plugin.
pub struct ExamplePluginMainThread<'a> {
    /// The host handle
    host: HostMainThreadHandle<'a>,
    // The host GUI extension handle
    host_gui: Option<HostGui>,
    /// The plugin's GUI state and context
    gui: RefCell<Option<ExamplePluginGui>>,
}

impl<'a> ExamplePluginMainThread<'a> {
    fn borrow_window(&self) -> Option<Ref<'_, baseview::Window>> {
        let gui = self.gui.borrow();
        gui.is_some().then(|| Ref::map(gui, |g| &g.as_ref().unwrap().handle))
    }
}

impl<'a> PluginMainThread<'a, ()> for ExamplePluginMainThread<'a> {
    fn on_main_thread(&self) {
        if let Some(gui) = self.gui.borrow().as_ref() {
            gui.handle.host_main_thread_callback();
        }
    }
}

impl PluginStateImpl for ExamplePluginMainThread<'_> {
    fn save(&self, _output: &mut OutputStream) -> Result<(), PluginError> {
        Ok(())
    }

    fn load(&self, _input: &mut InputStream) -> Result<(), PluginError> {
        Ok(())
    }
}

clack_export_entry!(SinglePluginEntry<ExamplePlugin>);
