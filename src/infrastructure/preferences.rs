use std::path::PathBuf;
use vjay_application::{
    ports::PortResult,
    preferences::{Preferences, SettingsStore},
};

pub struct JsonSettingsStore(pub PathBuf);
impl JsonSettingsStore {
    pub fn new(path: PathBuf) -> Self {
        Self(path)
    }
}
impl SettingsStore for JsonSettingsStore {
    fn load(&self) -> PortResult<Preferences> {
        if !self.0.exists() {
            return Ok(Preferences::default());
        }
        if std::fs::metadata(&self.0).map_err(|e| e.to_string())?.len() > 65536 {
            return Err("Settings file exceeds 64 KiB".into());
        }
        let bytes = std::fs::read(&self.0).map_err(|e| e.to_string())?;
        if bytes.len() > 65536 {
            return Err("Settings file exceeds 64 KiB".into());
        }
        let prefs: Preferences = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
        prefs.validate()?;
        Ok(prefs)
    }
    fn save(&self, prefs: &Preferences) -> PortResult<()> {
        prefs.validate()?;
        if let Some(parent) = self.0.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        crate::storage::atomic_write(
            &self.0,
            &serde_json::to_vec_pretty(prefs).map_err(|e| e.to_string())?,
        )
    }
}
pub fn default_path() -> PathBuf {
    if let Some(path) = std::env::var_os("VJAY_SETTINGS_PATH") {
        return path.into();
    }
    #[cfg(target_os = "macos")]
    {
        PathBuf::from(std::env::var_os("HOME").unwrap_or_default())
            .join("Library/Application Support/VJAY/settings.json")
    }
    #[cfg(target_os = "windows")]
    {
        PathBuf::from(std::env::var_os("APPDATA").unwrap_or_default()).join("VJAY/settings.json")
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".config")
            })
            .join("vjay/settings.json")
    }
}

#[cfg(test)]
mod tests;
