use super::types::*;
use crate::utils::{Error, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Config {
    pub app: AppConfig,
    pub tools: ToolsConfig,
    pub logging: LoggingConfig,
    pub progress: ProgressConfig,
    pub analysis: AnalysisConfig,
    pub profiles: HashMap<String, RawProfile>,
    pub filters: FiltersConfig,
}

impl Config {
    pub fn load<P: AsRef<Path>>(config_path: P) -> Result<Self> {
        let config_str = std::fs::read_to_string(config_path)?;
        let config: Config = serde_yaml::from_str(&config_str)?;
        config.validate()?;
        Ok(config)
    }

    pub fn load_default() -> Result<Self> {
        Self::load("config.yaml")
    }

    fn validate(&self) -> Result<()> {
        if self.progress.update_interval_ms == 0 {
            return Err(Error::validation(
                "update_interval_ms must be greater than 0",
            ));
        }

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

            if profile.base_bitrate == 0 {
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
        Self {
            app: AppConfig {
                temp_dir: "/tmp".to_string(),
                stats_prefix: "ffmpeg_stats".to_string(),
            },
            tools: ToolsConfig {
                ffmpeg: "ffmpeg".to_string(),
                ffprobe: "ffprobe".to_string(),
                nnedi_weights: None,
                dovi_tool: None,
            },
            logging: LoggingConfig {
                level: "info".to_string(),
                show_timestamps: true,
                colored_output: true,
            },
            progress: ProgressConfig {
                update_interval_ms: 1000,
            },
            analysis: AnalysisConfig {
                crop_detection: CropDetectionConfig {
                    enabled: true,
                    sample_count: 3,
                    sdr_crop_limit: 24,
                    hdr_crop_limit: 64,
                    min_pixel_change_percent: 1.0,
                },
                hdr_detection: HdrDetectionConfig {
                    enabled: true,
                    color_space_patterns: vec!["bt2020".to_string(), "rec2020".to_string()],
                    transfer_patterns: vec!["smpte2084".to_string(), "arib-std-b67".to_string()],
                    crf_adjustment: 2.0,
                },
                hdr: Some(UnifiedHdrConfig::default()),
                dolby_vision: Some(DolbyVisionConfig::default()),
                hdr10_plus: Some(crate::config::types::Hdr10PlusConfig::default()),
            },
            profiles: {
                let mut profiles = HashMap::new();
                let mut x265_params = HashMap::new();
                x265_params.insert(
                    "preset".to_string(),
                    serde_yaml::Value::String("medium".to_string()),
                );
                profiles.insert(
                    "default".to_string(),
                    RawProfile {
                        title: "Default Profile".to_string(),
                        base_crf: 23.0,
                        base_bitrate: 5000,
                        hdr_bitrate: 6000,
                        content_type: "film".to_string(),
                        x265_params,
                    },
                );
                profiles
            },
            filters: FiltersConfig {
                deinterlace: DeinterlaceConfig {
                    primary_method: "nnedi".to_string(),
                    fallback_method: "yadif".to_string(),
                    nnedi_settings: NnediSettings {
                        field: "auto".to_string(),
                    },
                },
                denoise: DenoiseConfig {
                    filter: "hqdn3d".to_string(),
                    params: "1:1:2:2".to_string(),
                },
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    // Unused imports removed

    #[test]
    fn test_config_validation() {
        let mut config = Config::default();
        match config.validate() {
            Ok(()) => {}
            Err(e) => panic!("Config validation failed: {}", e),
        }

        // Test validation with invalid update_interval_ms
        config.progress.update_interval_ms = 0;
        assert!(config.validate().is_err());
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

progress:
  update_interval_ms: 500

analysis:
  crop_detection:
    enabled: false
    sample_count: 1
    sdr_crop_limit: 24
    hdr_crop_limit: 64
    min_pixel_change_percent: 1.0
  hdr_detection:
    enabled: true
    color_space_patterns: ["bt2020"]
    transfer_patterns: ["smpte2084"]
    crf_adjustment: 2.0


profiles:
  test:
    title: "Test Profile"
    base_crf: 22.0
    base_bitrate: 5000
    hdr_bitrate: 6000
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
        assert_eq!(config.progress.update_interval_ms, 500);
        // web_search was removed from config
    }
}
