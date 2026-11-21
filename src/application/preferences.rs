use crate::ports::PortResult;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AudioSettings {
    pub device: String,
    pub sample_rate: u32,
    pub buffer_frames: u32,
    pub channel_pair: u16,
    pub volume: f32,
}
impl Default for AudioSettings {
    fn default() -> Self {
        Self {
            device: String::new(),
            sample_rate: 0,
            buffer_frames: 0,
            channel_pair: 0,
            volume: 0.8,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct VideoSettings {
    pub width: u32,
    pub height: u32,
    pub frame_rate: u32,
    pub display: String,
    pub fullscreen: bool,
    pub vsync: bool,
}
impl Default for VideoSettings {
    fn default() -> Self {
        Self {
            width: 1280,
            height: 720,
            frame_rate: 60,
            display: String::new(),
            fullscreen: false,
            vsync: true,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Preferences {
    pub audio: AudioSettings,
    pub video: VideoSettings,
    pub default_project_name: String,
}
impl Default for Preferences {
    fn default() -> Self {
        Self {
            audio: Default::default(),
            video: Default::default(),
            default_project_name: "Untitled.vjay.json".into(),
        }
    }
}
impl Preferences {
    pub fn validate(&self) -> PortResult<()> {
        crate::ports::Extent::new(self.video.width, self.video.height)?;
        if !(15..=240).contains(&self.video.frame_rate) {
            return Err("Frame rate must be 15–240 Hz".into());
        }
        let a = &self.audio;
        if a.device.len() > 1024
            || self.video.display.len() > 256
            || !a.volume.is_finite()
            || !(0.0..=1.0).contains(&a.volume)
            || a.channel_pair > 31
            || (a.sample_rate != 0 && !(8000..=192000).contains(&a.sample_rate))
            || (a.buffer_frames != 0 && !(32..=8192).contains(&a.buffer_frames))
        {
            return Err("Invalid audio output settings".into());
        }
        let name = self.default_project_name.trim();
        if name.is_empty()
            || name.len() > 120
            || name == "."
            || name == ".."
            || name
                .chars()
                .any(|c| c.is_control() || matches!(c, '/' | '\\' | ':'))
        {
            return Err("Default project name must be a file name without path separators".into());
        }
        Ok(())
    }
}
pub trait SettingsStore: Send + Sync {
    fn load(&self) -> PortResult<Preferences>;
    fn save(&self, settings: &Preferences) -> PortResult<()>;
}
#[derive(Default)]
pub struct MemorySettings(std::sync::Mutex<Preferences>);
impl SettingsStore for MemorySettings {
    fn load(&self) -> PortResult<Preferences> {
        self.0.lock().map(|s| s.clone()).map_err(|e| e.to_string())
    }
    fn save(&self, settings: &Preferences) -> PortResult<()> {
        settings.validate()?;
        *self.0.lock().map_err(|e| e.to_string())? = settings.clone();
        Ok(())
    }
}

#[derive(Clone, Debug)]
pub struct VideoSource {
    pub id: String,
    pub name: String,
}
