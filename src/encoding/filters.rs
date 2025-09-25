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
            // Check if we have crop filter - if so, use filter_complex for better performance
            let has_crop = self.filters.iter().any(|f| f.starts_with("crop="));
            if has_crop {
                // Use filter_complex with explicit input/output mapping for better stream handling
                let filter_spec = format!("[0:v]{}[v]", self.filters.join(","));
                vec!["-filter_complex".to_string(), filter_spec]
            } else {
                vec!["-vf".to_string(), self.filters.join(",")]
            }
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

        // Try to use primary method (NNEDI) if weights file is available
        let filter = if deinterlace_config.primary_method == "nnedi" {
            if let Some(weights_path) = &self.config.tools.nnedi_weights {
                // Check if weights file exists
                if std::path::Path::new(weights_path).exists() {
                    let field_mode = &deinterlace_config.nnedi_settings.field;
                    let field_value = match field_mode.as_str() {
                        "af" => -2,   // use frame flags, both fields
                        "a" => -1,    // use frame flags, single field
                        "t" => 0,     // use top field only
                        "b" => 1,     // use bottom field only
                        "tf" => 2,    // use both fields, top first
                        "bf" => 3,    // use both fields, bottom first
                        "auto" => -1, // fallback to 'a' for auto
                        _ => {
                            tracing::warn!(
                                "Unknown NNEDI field mode '{}', using 'a' (-1)",
                                field_mode
                            );
                            -1
                        }
                    };
                    format!("nnedi=weights={}:field={}", weights_path, field_value)
                } else {
                    // Fall back to fallback method if weights file doesn't exist
                    tracing::warn!(
                        "NNEDI weights file not found at: {}, falling back to {}",
                        weights_path,
                        deinterlace_config.fallback_method
                    );
                    self.build_fallback_deinterlace_filter(&deinterlace_config.fallback_method)?
                }
            } else {
                // Fall back to fallback method if no weights path configured
                tracing::warn!(
                    "No NNEDI weights path configured, falling back to {}",
                    deinterlace_config.fallback_method
                );
                self.build_fallback_deinterlace_filter(&deinterlace_config.fallback_method)?
            }
        } else {
            // Use the configured primary method (not NNEDI)
            self.build_fallback_deinterlace_filter(&deinterlace_config.primary_method)?
        };

        Ok(filter)
    }

    fn build_fallback_deinterlace_filter(&self, method: &str) -> Result<String> {
        match method {
            "yadif" => Ok("yadif=mode=send_field:parity=auto:deint=interlaced".to_string()),
            "bwdif" => Ok("bwdif=mode=send_field:parity=auto:deint=interlaced".to_string()),
            other => Err(Error::encoding(format!(
                "Unsupported deinterlace method: {}",
                other
            ))),
        }
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
                dovi_tool: None,
                hdr10plus_tool: None,
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
                crop_detection: CropDetectionConfig::default(),
                hdr_detection: HdrDetectionConfig {
                    enabled: true,
                    passthrough_mode: false,
                    color_space_patterns: vec!["bt2020".to_string()],
                    transfer_patterns: vec!["smpte2084".to_string()],
                    crf_adjustment: 2.0,
                },
                hdr: Some(crate::config::UnifiedHdrConfig::default()),
                dolby_vision: Some(crate::config::DolbyVisionConfig::default()),
                hdr10_plus: Some(crate::config::Hdr10PlusConfig::default()),
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
            stream_selection: None,
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
        chain.add_filter("crop=1920:800:0:140".to_string());
        chain.add_filter("hqdn3d=1:1:2:2".to_string());

        assert!(!chain.is_empty());
        // With crop filter, it should use filter_complex
        assert_eq!(
            chain.build_ffmpeg_args(),
            vec![
                "-filter_complex",
                "[0:v]crop=1920:800:0:140,hqdn3d=1:1:2:2[v]"
            ]
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

    #[test]
    fn test_nnedi_filter_construction() {
        let mut config = create_test_config();
        // Set up NNEDI weights file path for the test
        let weights_path = "/tmp/test_weights.bin";
        config.tools.nnedi_weights = Some(weights_path.to_string());

        // Create a dummy weights file for the test
        std::fs::write(weights_path, "test").unwrap();

        let builder = FilterBuilder::new(&config);
        let filter_result = builder.build_deinterlace_filter();

        assert!(filter_result.is_ok());
        let filter = filter_result.unwrap();

        // Should contain the NNEDI filter with correct field mapping
        assert!(filter.contains("nnedi="));
        assert!(filter.contains(&format!("weights={}", weights_path)));
        assert!(filter.contains("field=-1")); // "auto" should map to -1

        // Clean up test file
        let _ = std::fs::remove_file(weights_path);
    }

    #[test]
    fn test_nnedi_field_mapping() {
        let mut config = create_test_config();
        config.tools.nnedi_weights = Some("/tmp/test_weights.bin".to_string());

        // Create a dummy weights file for the test
        std::fs::write("/tmp/test_weights.bin", "test").unwrap();

        // Test "af" mapping to -2
        config.filters.deinterlace.nnedi_settings.field = "af".to_string();
        let builder = FilterBuilder::new(&config);
        let filter = builder.build_deinterlace_filter().unwrap();
        assert!(filter.contains("field=-2"));

        // Test "a" mapping to -1
        config.filters.deinterlace.nnedi_settings.field = "a".to_string();
        let builder = FilterBuilder::new(&config);
        let filter = builder.build_deinterlace_filter().unwrap();
        assert!(filter.contains("field=-1"));

        // Test "t" mapping to 0
        config.filters.deinterlace.nnedi_settings.field = "t".to_string();
        let builder = FilterBuilder::new(&config);
        let filter = builder.build_deinterlace_filter().unwrap();
        assert!(filter.contains("field=0"));

        // Clean up test file
        let _ = std::fs::remove_file("/tmp/test_weights.bin");
    }
}
