use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use crate::utils::{Result, Error};
use super::types::*;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Config {
    pub app: AppConfig,
    pub tools: ToolsConfig,
    pub logging: LoggingConfig,
    pub progress: ProgressConfig,
    pub analysis: AnalysisConfig,
    pub web_search: WebSearchConfig,
    pub content_classification: ContentClassificationConfig,
    pub profiles: HashMap<String, RawProfile>,
    pub content_adaptation: ContentAdaptationConfig,
    pub hardware: HardwareConfig,
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
        if self.app.max_concurrent_jobs == 0 {
            return Err(Error::validation("max_concurrent_jobs must be greater than 0"));
        }

        if self.progress.update_interval_ms == 0 {
            return Err(Error::validation("update_interval_ms must be greater than 0"));
        }

        if self.progress.stall_detection_seconds == 0 {
            return Err(Error::validation("stall_detection_seconds must be greater than 0"));
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

            if ContentType::from_str(&profile.content_type).is_none() {
                return Err(Error::validation(format!(
                    "Invalid content_type for profile '{}': {}",
                    name, profile.content_type
                )));
            }
        }

        Ok(())
    }

    pub fn get_crf_modifier(&self, content_type: ContentType) -> f32 {
        self.content_adaptation
            .crf_modifiers
            .get(content_type.as_str())
            .copied()
            .unwrap_or(0.0)
    }

    pub fn get_bitrate_multiplier(&self, content_type: ContentType) -> f32 {
        self.content_adaptation
            .bitrate_multipliers
            .get(content_type.as_str())
            .copied()
            .unwrap_or(1.0)
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            app: AppConfig {
                temp_dir: "/tmp".to_string(),
                stats_prefix: "ffmpeg_stats".to_string(),
                max_concurrent_jobs: 1,
            },
            tools: ToolsConfig {
                ffmpeg: "ffmpeg".to_string(),
                ffprobe: "ffprobe".to_string(),
                nnedi_weights: None,
            },
            logging: LoggingConfig {
                level: "info".to_string(),
                show_timestamps: true,
                colored_output: true,
            },
            progress: ProgressConfig {
                update_interval_ms: 1000,
                stall_detection_seconds: 15,
                show_eta: true,
                show_file_size: true,
            },
            analysis: AnalysisConfig {
                complexity_analysis: ComplexityAnalysisConfig {
                    enabled: true,
                    sample_points: vec![0.1, 0.25, 0.5, 0.75, 0.9],
                    methods: vec![
                        "high_frequency".to_string(),
                        "local_variance".to_string(),
                        "edge_detection".to_string(),
                        "dark_scene".to_string(),
                    ],
                },
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
            },
            web_search: WebSearchConfig {
                enabled: true,
                timeout_seconds: 10,
                user_agent: "FFmpeg-Autoencoder/3.0".to_string(),
                simulation_mode: false,
            },
            content_classification: ContentClassificationConfig {
                grain_thresholds: ThresholdConfig {
                    low: 20,
                    medium: 50,
                    high: 80,
                },
                motion_thresholds: ThresholdConfig {
                    low: 10,
                    medium: 30,
                    high: 60,
                },
                scene_change_thresholds: ThresholdConfig {
                    low: 5,
                    medium: 15,
                    high: 25,
                },
            },
            profiles: HashMap::new(),
            content_adaptation: ContentAdaptationConfig {
                crf_modifiers: HashMap::new(),
                bitrate_multipliers: HashMap::new(),
            },
            hardware: HardwareConfig {
                cuda: CudaConfig {
                    enabled: false,
                    fallback_to_software: true,
                    decode_acceleration: true,
                    filter_acceleration: true,
                },
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
                    hardware_variant: "nlmeans".to_string(),
                },
                scale: ScaleConfig {
                    algorithm: "lanczos".to_string(),
                    preserve_aspect_ratio: true,
                },
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;
    use std::io::Write;

    #[test]
    fn test_config_validation() {
        let mut config = Config::default();
        assert!(config.validate().is_ok());

        config.app.max_concurrent_jobs = 0;
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_config_load_from_string() {
        let yaml = r#"
app:
  temp_dir: "/tmp"
  stats_prefix: "test"
  max_concurrent_jobs: 2

tools:
  ffmpeg: "ffmpeg"
  ffprobe: "ffprobe"

logging:
  level: "debug"
  show_timestamps: false
  colored_output: true

progress:
  update_interval_ms: 500
  stall_detection_seconds: 30
  show_eta: true
  show_file_size: false

analysis:
  complexity_analysis:
    enabled: true
    sample_points: [0.1, 0.5, 0.9]
    methods: ["high_frequency"]
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

web_search:
  enabled: false
  timeout_seconds: 5
  user_agent: "test"
  simulation_mode: true

content_classification:
  grain_thresholds:
    low: 10
    medium: 25
    high: 50
  motion_thresholds:
    low: 5
    medium: 15
    high: 30
  scene_change_thresholds:
    low: 2
    medium: 8
    high: 15

profiles:
  test:
    title: "Test Profile"
    base_crf: 22.0
    base_bitrate: 5000
    hdr_bitrate: 6000
    content_type: "film"
    x265_params:
      preset: "medium"

content_adaptation:
  crf_modifiers:
    film: 0.0
  bitrate_multipliers:
    film: 1.0

hardware:
  cuda:
    enabled: false
    fallback_to_software: true
    decode_acceleration: true
    filter_acceleration: true

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
    hardware_variant: "nlmeans"
  crop:
    auto_detect: true
    validation:
      min_change_percent: 1.0
      temporal_samples: 3
  scale:
    algorithm: "lanczos"
    preserve_aspect_ratio: true
"#;

        let config: Config = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.app.max_concurrent_jobs, 2);
        assert_eq!(config.logging.level, "debug");
        assert!(!config.logging.show_timestamps);
        assert_eq!(config.progress.update_interval_ms, 500);
        assert!(!config.web_search.enabled);
    }
}