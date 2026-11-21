use super::*;
impl CpalAudio {
    pub(super) fn configure_output(
        &mut self,
        settings: &vjay_application::preferences::AudioSettings,
    ) -> PortResult<()> {
        let candidate = vjay_application::preferences::Preferences {
            audio: settings.clone(),
            ..Default::default()
        };
        candidate.validate()?;
        if !settings.device.is_empty()
            || settings.sample_rate != 0
            || settings.buffer_frames != 0
            || settings.channel_pair != 0
        {
            device::configuration(settings)?;
        }
        let changed = self.output.device != settings.device
            || self.output.sample_rate != settings.sample_rate
            || self.output.buffer_frames != settings.buffer_frames
            || self.output.channel_pair != settings.channel_pair;
        self.master_volume
            .store(settings.volume.to_bits(), Ordering::Release);
        if changed {
            self.playing.store(false, Ordering::Release);
            self.stream = None;
            self.keys.clear();
        }
        self.output = settings.clone();
        self.device_name = settings.device.clone();
        Ok(())
    }
}
