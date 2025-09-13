use crate::config::Config;
use crate::utils::{Error, Result};

#[derive(Debug, Clone, Default)]
pub struct FilterChain {
    filters: Vec<String>,
}

impl FilterChain {
    pub fn new() -> Self {
        Self {
            filters: Vec::new(),
        }
    }

    pub fn add_filter(&mut self, filter: String) {
        self.filters.push(filter);
    }

    pub fn build_ffmpeg_args(&self) -> Vec<String> {
        if self.filters.is_empty() {
            Vec::new()
        } else {
            vec!["-vf".to_string(), self.filters.join(",")]
        }
    }

    pub fn is_empty(&self) -> bool {
        self.filters.is_empty()
    }
}

impl std::fmt::Display for FilterChain {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.filters.is_empty() {
            write!(f, "None")
        } else {
            write!(f, "{}", self.filters.join(","))
        }
    }
}

pub struct FilterBuilder<'a> {
    config: &'a Config,
    chain: FilterChain,
}

impl<'a> FilterBuilder<'a> {
    pub fn new(config: &'a Config) -> Self {
        Self {
            config,
            chain: FilterChain::new(),
        }
    }

    /// Build complete filter chain in correct processing order:
    /// 1. Deinterlacing (NNEDI/yadif)
    /// 2. Denoising (hqdn3d)
    /// 3. Cropping (manual override or auto-detection)
    pub fn build_complete_chain(
        mut self,
        deinterlace: bool,
        denoise: bool,
        crop: Option<&str>,
    ) -> Result<FilterChain> {
        // Step 1: Optional Deinterlacing (first in pipeline)
        if deinterlace {
            let filter = self.build_deinterlace_filter()?;
            self.chain.add_filter(filter);
        }

        // Step 2: Optional Denoising (second in pipeline)
        if denoise {
            let filter = self.build_denoise_filter();
            self.chain.add_filter(filter);
        }

        // Step 3: Cropping (third in pipeline)
        if let Some(crop_value) = crop {
            let filter = format!("crop={}", crop_value);
            self.chain.add_filter(filter);
        }

        Ok(self.chain)
    }

    pub fn with_deinterlace(mut self, enabled: bool) -> Result<Self> {
        if enabled {
            let filter = self.build_deinterlace_filter()?;
            self.chain.add_filter(filter);
        }
        Ok(self)
    }

    pub fn with_denoise(mut self, enabled: bool) -> Self {
        if enabled {
            let filter = self.build_denoise_filter();
            self.chain.add_filter(filter);
        }
        self
    }

    pub fn with_crop(mut self, crop: Option<&str>) -> Result<Self> {
        if let Some(crop_value) = crop {
            let filter = format!("crop={}", crop_value);
            self.chain.add_filter(filter);
        }
        Ok(self)
    }

    pub fn build(self) -> FilterChain {
        self.chain
    }

    fn build_deinterlace_filter(&self) -> Result<String> {
        let deinterlace_config = &self.config.filters.deinterlace;

        // Use fallback method (yadif) for simplicity since NNEDI requires weights file
        let filter = match deinterlace_config.fallback_method.as_str() {
            "yadif" => "yadif=mode=send_field:parity=auto:deint=interlaced".to_string(),
            "bwdif" => "bwdif=mode=send_field:parity=auto:deint=interlaced".to_string(),
            other => {
                return Err(Error::encoding(format!(
                    "Unsupported deinterlace method: {}",
                    other
                )));
            }
        };

        Ok(filter)
    }

    fn build_denoise_filter(&self) -> String {
        let denoise_config = &self.config.filters.denoise;

        // Use software denoising only
        format!("{}={}", denoise_config.filter, denoise_config.params)
    }
}

// Helper function to validate crop format
pub fn validate_crop_format(crop: &str) -> Result<()> {
    let parts: Vec<&str> = crop.split(':').collect();
    if parts.len() != 4 {
        return Err(Error::encoding("Crop format must be width:height:x:y"));
    }

    for part in parts {
        if part.parse::<u32>().is_err() {
            return Err(Error::encoding("All crop values must be positive integers"));
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::types::*;
    use std::collections::HashMap;

    fn create_test_config() -> Config {
        Config {
            app: AppConfig {
                temp_dir: "/tmp".to_string(),
                stats_prefix: "test".to_string(),
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
                crop_detection: CropDetectionConfig::default(),
                hdr_detection: HdrDetectionConfig {
                    enabled: true,
                    color_space_patterns: vec!["bt2020".to_string()],
                    transfer_patterns: vec!["smpte2084".to_string()],
                    crf_adjustment: 2.0,
                },
            },
            profiles: HashMap::new(),
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

    #[test]
    fn test_empty_filter_chain() {
        let chain = FilterChain::new();
        assert!(chain.is_empty());
        assert_eq!(chain.build_ffmpeg_args(), Vec::<String>::new());
    }

    #[test]
    fn test_filter_chain_with_filters() {
        let mut chain = FilterChain::new();
        chain.add_filter("scale=1920:1080".to_string());
        chain.add_filter("hqdn3d=1:1:2:2".to_string());

        assert!(!chain.is_empty());
        assert_eq!(
            chain.build_ffmpeg_args(),
            vec!["-vf", "scale=1920:1080,hqdn3d=1:1:2:2"]
        );
    }

    #[test]
    fn test_validate_crop_format() {
        assert!(validate_crop_format("1920:800:0:140").is_ok());
        assert!(validate_crop_format("invalid").is_err());
        assert!(validate_crop_format("1920:800:0").is_err());
        assert!(validate_crop_format("1920:800:0:invalid").is_err());
    }

    #[test]
    fn test_filter_builder_with_all_options() {
        let config = create_test_config();
        let builder = FilterBuilder::new(&config);

        let result = builder.build_complete_chain(
            true,                   // deinterlace
            true,                   // denoise
            Some("1920:800:0:140"), // crop
        );

        assert!(result.is_ok());
        let chain = result.unwrap();
        assert!(!chain.is_empty());
    }
}
