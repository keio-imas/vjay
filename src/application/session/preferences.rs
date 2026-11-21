use super::*;
use crate::preferences::Preferences;

impl Session {
    pub fn preferences(&self) -> &Preferences {
        &self.preferences
    }
    pub fn load_preferences(&mut self) -> PortResult<()> {
        let prefs = self.settings_store.load()?;
        prefs.validate()?;
        let audio = self.audio.configure(&prefs.audio);
        self.renderer.set_vsync(prefs.video.vsync)?;
        self.preferences = prefs;
        self.audio_error = audio.as_ref().err().cloned();
        audio
    }
    pub fn apply_preferences(&mut self, prefs: Preferences) -> PortResult<()> {
        prefs.validate()?;
        let previous = self.preferences.clone();
        self.audio.configure(&prefs.audio)?;
        if let Err(e) = self
            .renderer
            .set_vsync(prefs.video.vsync)
            .and_then(|_| self.settings_store.save(&prefs))
        {
            let _ = self.audio.configure(&previous.audio);
            let _ = self.renderer.set_vsync(previous.video.vsync);
            self.invalidate(true);
            return Err(e);
        }
        self.set_extent(Extent::new(prefs.video.width, prefs.video.height)?)?;
        self.preferences = prefs;
        self.audio_error = None;
        self.invalidate(true);
        Ok(())
    }
    pub fn frame_interval(&self) -> std::time::Duration {
        std::time::Duration::from_secs_f64(1.0 / self.preferences.video.frame_rate as f64)
    }
    pub fn video_source_info(&self, provider: &str) -> Vec<crate::preferences::VideoSource> {
        self.renderer.video_source_info(provider)
    }
}
