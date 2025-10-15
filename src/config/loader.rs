use super::types::*;
use crate::utils::{Error, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Discovers the config file location in the following priority order:
/// 1. If explicit_path is Some and exists, use it
/// 2. Check for config/config.yaml next to the binary
/// 3. Check user config directory (~/.config/ffmpeg-encoder/ on Linux,
///    ~/Library/Application Support/ffmpeg-encoder/ on macOS,
///    %APPDATA%\ffmpeg-encoder\ on Windows)
/// 4. Fall back to default config
pub fn discover_config_path(explicit_path: Option<&Path>) -> Option<PathBuf> {
    // Priority 1: Explicit path provided via CLI argument
    if let Some(path) = explicit_path {
        if path.exists() {
            tracing::debug!("Using explicit config path: {}", path.display());
            return Some(path.to_path_buf());
        } else {
            tracing::debug!("Explicit config path does not exist: {}", path.display());
        }
    }

    // Priority 2: Check for config/config.yaml next to the binary
    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(exe_dir) = exe_path.parent() {
            let binary_config = exe_dir.join("config").join("config.yaml");
            if binary_config.exists() {
                tracing::debug!("Found config next to binary: {}", binary_config.display());
                return Some(binary_config);
            }
            tracing::debug!("No config found at: {}", binary_config.display());
        }
    }

    // Priority 3: Check user config directory (cross-platform)
    if let Some(config_dir) = dirs::config_dir() {
        let user_config = config_dir.join("ffmpeg-encoder").join("config.yaml");
        if user_config.exists() {
            tracing::debug!("Found config in user directory: {}", user_config.display());
            return Some(user_config);
        }
        tracing::debug!("No config found at: {}", user_config.display());
    }

    // Priority 4: No config found, will fall back to default
    tracing::debug!("No config file found, will use default configuration");
    None
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Config {
    pub app: AppConfig,
    pub tools: ToolsConfig,
    pub logging: LoggingConfig,
    pub analysis: AnalysisConfig,
    pub profiles: HashMap<String, RawProfile>,
    pub filters: FiltersConfig,
    #[serde(default)]
    pub stream_selection_profiles: HashMap<String, RawStreamSelectionProfile>,
    #[serde(default)]
    pub preview_profiles: HashMap<String, RawPreviewProfile>,
}

impl Config {
    pub fn load<P: AsRef<Path>>(config_path: P) -> Result<Self> {
        let config_str = std::fs::read_to_string(config_path)?;
        let config: Config = serde_yaml::from_str(&config_str)?;
        config.validate()?;
        Ok(config)
    }

    pub fn load_with_fallback<P: AsRef<Path>>(config_path: P) -> Result<Self> {
        // Try to load the specified config file
        match Self::load(&config_path) {
            Ok(config) => Ok(config),
            Err(_) => {
                // If the specified config doesn't exist, try default configs
                Self::load_default()
            }
        }
    }

    /// Loads config with automatic discovery.
    /// If explicit_path is provided and exists, it takes priority.
    /// Otherwise, searches in: binary dir, user config dir, then falls back to default.
    pub fn load_with_discovery(explicit_path: Option<&Path>) -> Result<Self> {
        if let Some(config_path) = discover_config_path(explicit_path) {
            tracing::info!("Loading configuration from: {}", config_path.display());
            Self::load(&config_path)
        } else {
            tracing::info!("No config file found, using default configuration");
            Self::load_default()
        }
    }

    pub fn load_default() -> Result<Self> {
        // Try to load external config.default.yaml first
        let default_paths = ["config.default.yaml", "./config/config.default.yaml"];

        for path in &default_paths {
            if std::path::Path::new(path).exists() {
                match Self::load(path) {
                    Ok(config) => return Ok(config),
                    Err(_) => continue, // Try next path or fall back to embedded
                }
            }
        }

        // Fall back to embedded default configuration
        let default_config_str = include_str!("../../config/config.default.yaml");
        let config: Config = serde_yaml::from_str(default_config_str)?;
        config.validate()?;
        Ok(config)
    }

    fn validate(&self) -> Result<()> {
        if self.profiles.is_empty() {
            return Err(Error::validation("At least one profile must be defined"));
        }

        for (name, profile) in &self.profiles {
            if profile.base_crf <= 0.0 || profile.base_crf > 51.0 {
                return Err(Error::validation(format!(
                    "Invalid CRF value for profile '{}': {} (must be between 0 and 51)",
                    name, profile.base_crf
                )));
            }

            if profile.bitrate == 0 {
                return Err(Error::validation(format!(
                    "Invalid base_bitrate for profile '{}': must be greater than 0",
                    name
                )));
            }

            if ContentType::from_string(&profile.content_type).is_none() {
                return Err(Error::validation(format!(
                    "Invalid content_type for profile '{}': {}",
                    name, profile.content_type
                )));
            }
        }

        Ok(())
    }
}

impl Default for Config {
    fn default() -> Self {
        Self::load_default().expect("Failed to load default configuration")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    // Unused imports removed

    #[test]
    fn test_config_validation() {
        let config = Config::default();
        match config.validate() {
            Ok(()) => {}
            Err(e) => panic!("Config validation failed: {}", e),
        }

        // Test validation - all checks pass for default config
    }

    #[test]
    fn test_config_load_from_string() {
        let yaml = r#"
app:
  temp_dir: "/tmp"
  stats_prefix: "test"

tools:
  ffmpeg: "ffmpeg"
  ffprobe: "ffprobe"

logging:
  level: "debug"
  show_timestamps: false
  colored_output: true


analysis:
  crop_detection:
    enabled: false
    sample_count: 1
    sdr_crop_limit: 24
    hdr_crop_limit: 64
    min_pixel_change_percent: 1.0
  hdr:
    enabled: true
    crf_adjustment: 2.0
    bitrate_multiplier: 1.3


profiles:
  test:
    title: "Test Profile"
    base_crf: 22.0
    bitrate: 5000
    content_type: "film"
    x265_params:
      preset: "medium"

filters:
  deinterlace:
    primary_method: "yadif"
    fallback_method: "yadif"
    nnedi_settings:
      field: "auto"
      weights: "test.bin"
  denoise:
    filter: "hqdn3d"
    params: "1:1:2:2"
  crop:
    auto_detect: true
    validation:
      min_change_percent: 1.0
      temporal_samples: 3
"#;

        let config: Config = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.logging.level, "debug");
        assert!(!config.logging.show_timestamps);
        // web_search was removed from config
    }
}
