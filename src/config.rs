use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NitroConfig {
    pub mode: String,
    pub fan_mode: String,
    pub cpu_fan_speed: u8,
    pub gpu_fan_speed: u8,
    pub battery_limiter: bool,
    pub backlight_timeout: bool,
    pub keyboard_brightness: u8,
}

impl Default for NitroConfig {
    fn default() -> Self {
        Self {
            mode: "balanced".to_string(),
            fan_mode: "auto".to_string(),
            cpu_fan_speed: 50,
            gpu_fan_speed: 50,
            battery_limiter: true,
            backlight_timeout: true,
            keyboard_brightness: 100,
        }
    }
}

impl NitroConfig {
    pub fn config_path() -> PathBuf {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        PathBuf::from(home).join(".config/nitrosense/config.json")
    }

    pub fn load() -> Self {
        let path = Self::config_path();
        if let Ok(content) = fs::read_to_string(&path) {
            if let Ok(cfg) = serde_json::from_str(&content) {
                return cfg;
            }
        }
        Self::default()
    }

    pub fn save(&self) -> Result<(), std::io::Error> {
        let path = Self::config_path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let data = serde_json::to_string_pretty(self)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
        fs::write(path, data)?;
        Ok(())
    }
}
