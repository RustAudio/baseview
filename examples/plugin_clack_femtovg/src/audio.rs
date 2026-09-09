use crate::ExamplePluginMainThread;
use clack_plugin::prelude::*;

pub struct ExamplePluginAudioProcessor;

impl<'a> PluginAudioProcessor<'a, (), ExamplePluginMainThread<'a>> for ExamplePluginAudioProcessor {
    fn activate(
        _host: HostAudioProcessorHandle<'a>, _main_thread: &mut ExamplePluginMainThread,
        _shared: &'a (), _audio_config: PluginAudioConfiguration,
    ) -> Result<Self, PluginError> {
        Ok(Self)
    }

    fn process(
        &mut self, _process: Process, mut audio: Audio, _events: Events,
    ) -> Result<ProcessStatus, PluginError> {
        for mut port in audio.port_pairs() {
            let channels = port.channels()?.into_f32().expect("Expected f32 channels");

            for channel_pair in channels {
                match channel_pair {
                    ChannelPair::OutputOnly(o) => o.fill(0.0),
                    ChannelPair::InputOutput(i, o) => o.copy_from_slice(i),
                    _ => {}
                }
            }
        }

        Ok(ProcessStatus::Continue)
    }
}
