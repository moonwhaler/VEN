use std::path::Path;
use crate::utils::{Result, Error};
use crate::config::Config;
use crate::hardware::cuda::{CudaAccelerator, HardwareAcceleration};

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
    
    pub fn to_string(&self) -> String {
        if self.filters.is_empty() {
            "None".to_string()
        } else {
            self.filters.join(",")
        }
    }
}

pub struct FilterBuilder<'a> {
    config: &'a Config,
    chain: FilterChain,
    hardware_acceleration: Option<HardwareAcceleration>,
    cuda_accelerator: Option<&'a CudaAccelerator>,
}

impl<'a> FilterBuilder<'a> {
    pub fn new(
        config: &'a Config, 
        hardware_acceleration: Option<HardwareAcceleration>,
        cuda_accelerator: Option<&'a CudaAccelerator>,
    ) -> Self {
        Self {
            config,
            chain: FilterChain::new(),
            hardware_acceleration,
            cuda_accelerator,
        }
    }
    
    /// Build filter chain following exact bash implementation order:
    /// 1. Deinterlacing (NNEDI/yadif)
    /// 2. Denoising (hqdn3d)
    /// 3. Hardware acceleration (CUDA decode → hwdownload → CPU filters)
    /// 4. Cropping (manual override or auto-detection)
    /// 5. Scaling (resolution adjustment)
    pub fn build_complete_chain(
        mut self,
        deinterlace: bool,
        denoise: bool, 
        crop: Option<&str>,
        scale: Option<&str>,
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
        
        // Step 3: Hardware Acceleration handling
        if let (Some(accel), Some(cuda)) = (&self.hardware_acceleration, &self.cuda_accelerator) {
            if accel.uses_filter_acceleration() && cuda.get_capabilities().cuda_available {
                // For CUDA: decode → hwdownload → CPU filters → hwupload
                self.add_hardware_transition_filters();
            }
        }
        
        // Step 4: Cropping (fourth in pipeline)
        if let Some(crop_value) = crop {
            let filter = self.build_crop_filter(crop_value)?;
            self.chain.add_filter(filter);
        }
        
        // Step 5: Scaling (last in pipeline)
        if let Some(scale_value) = scale {
            let filter = self.build_scale_filter(scale_value)?;
            self.chain.add_filter(filter);
        }
        
        Ok(self.chain)
    }
    
    fn add_hardware_transition_filters(&mut self) {
        // Add hwdownload to transition from GPU to CPU for filters
        if !self.chain.filters.is_empty() {
            self.chain.add_filter("hwdownload".to_string());
            self.chain.add_filter("format=nv12".to_string());
        }
    }

    // Legacy methods for backward compatibility
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
            let filter = self.build_crop_filter(crop_value)?;
            self.chain.add_filter(filter);
        }
        Ok(self)
    }

    pub fn with_scale(mut self, scale: Option<&str>) -> Result<Self> {
        if let Some(scale_value) = scale {
            let filter = self.build_scale_filter(scale_value)?;
            self.chain.add_filter(filter);
        }
        Ok(self)
    }

    pub fn build(self) -> FilterChain {
        self.chain
    }

    fn build_deinterlace_filter(&self) -> Result<String> {
        let deinterlace_config = &self.config.filters.deinterlace;
        
        let primary_method = &deinterlace_config.primary_method;
        let fallback_method = &deinterlace_config.fallback_method;

        match primary_method.as_str() {
            "nnedi" => {
                if let Some(nnedi_weights) = &self.config.tools.nnedi_weights {
                    if Path::new(nnedi_weights).exists() {
                        let field = &deinterlace_config.nnedi_settings.field;
                        Ok(format!("nnedi=weights='{}':field={}", nnedi_weights, field))
                    } else {
                        tracing::warn!("NNEDI weights not found, falling back to {}", fallback_method);
                        self.build_fallback_deinterlace_filter(fallback_method)
                    }
                } else {
                    tracing::warn!("NNEDI weights path not configured, falling back to {}", fallback_method);
                    self.build_fallback_deinterlace_filter(fallback_method)
                }
            }
            "yadif" => Ok("yadif=1".to_string()),
            "bwdif" => Ok("bwdif=1".to_string()),
            _ => {
                tracing::warn!("Unknown deinterlace method: {}, falling back to yadif", primary_method);
                Ok("yadif=1".to_string())
            }
        }
    }

    fn build_fallback_deinterlace_filter(&self, method: &str) -> Result<String> {
        match method {
            "yadif" => Ok("yadif=1".to_string()),
            "bwdif" => Ok("bwdif=1".to_string()),
            _ => Ok("yadif=1".to_string()),
        }
    }

    fn build_denoise_filter(&self) -> String {
        let denoise_config = &self.config.filters.denoise;
        
        // Match bash implementation: hqdn3d=1:1:2:2 (light uniform grain reduction)
        if let Some(accel) = &self.hardware_acceleration {
            if accel.uses_filter_acceleration() && self.config.hardware.cuda.filter_acceleration {
                format!("{}={}", denoise_config.hardware_variant, denoise_config.params)
            } else {
                format!("{}={}", denoise_config.filter, denoise_config.params)
            }
        } else {
            format!("{}={}", denoise_config.filter, denoise_config.params)
        }
    }

    fn build_crop_filter(&self, crop: &str) -> Result<String> {
        let parts: Vec<&str> = crop.split(':').collect();
        if parts.len() != 4 {
            return Err(Error::validation(format!(
                "Invalid crop format: {} (expected width:height:x:y)",
                crop
            )));
        }

        let width: u32 = parts[0].parse()
            .map_err(|_| Error::validation("Invalid crop width"))?;
        let height: u32 = parts[1].parse()
            .map_err(|_| Error::validation("Invalid crop height"))?;
        let x: u32 = parts[2].parse()
            .map_err(|_| Error::validation("Invalid crop x offset"))?;
        let y: u32 = parts[3].parse()
            .map_err(|_| Error::validation("Invalid crop y offset"))?;

        Ok(format!("crop={}:{}:{}:{}", width, height, x, y))
    }

    fn build_scale_filter(&self, scale: &str) -> Result<String> {
        let parts: Vec<&str> = scale.split('x').collect();
        if parts.len() != 2 {
            return Err(Error::validation(format!(
                "Invalid scale format: {} (expected widthxheight)",
                scale
            )));
        }

        let width = parts[0];
        let height = parts[1];

        if !self.is_valid_scale_dimension(width) || !self.is_valid_scale_dimension(height) {
            return Err(Error::validation("Invalid scale dimensions"));
        }

        let algorithm = &self.config.filters.scale.algorithm;
        Ok(format!("scale={}:{}:flags={}", width, height, algorithm))
    }

    fn is_valid_scale_dimension(&self, dim: &str) -> bool {
        dim == "-1" || dim.parse::<u32>().is_ok()
    }
}

pub fn build_hardware_decode_args(config: &Config, _input_path: &str) -> Vec<String> {
    if !config.hardware.cuda.enabled || !config.hardware.cuda.decode_acceleration {
        return Vec::new();
    }

    vec![
        "-hwaccel".to_string(),
        "cuda".to_string(),
        "-hwaccel_output_format".to_string(),
        "cuda".to_string(),
    ]
}

pub fn add_hardware_filters_if_needed(filters: &mut FilterChain, config: &Config) {
    if config.hardware.cuda.enabled && !filters.is_empty() {
        let mut new_filters = Vec::new();
        new_filters.push("hwdownload".to_string());
        new_filters.push("format=nv12".to_string());
        
        for filter in &filters.filters {
            new_filters.push(filter.clone());
        }
        
        new_filters.push("hwupload_cuda".to_string());
        
        filters.filters = new_filters;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{Config, FiltersConfig, DeinterlaceConfig, NnediSettings, DenoiseConfig, ScaleConfig, CropConfig, CropValidationConfig};

    fn create_test_config() -> Config {
        let mut config = Config::default();
        config.filters = FiltersConfig {
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
            crop: CropConfig {
                auto_detect: true,
                validation: CropValidationConfig {
                    min_change_percent: 1.0,
                    temporal_samples: 3,
                },
            },
            scale: ScaleConfig {
                algorithm: "lanczos".to_string(),
                preserve_aspect_ratio: true,
            },
        };
        config
    }

    #[test]
    fn test_filter_chain_basic() {
        let mut chain = FilterChain::new();
        assert!(chain.is_empty());
        
        chain.add_filter("yadif=1".to_string());
        assert!(!chain.is_empty());
        
        let args = chain.build_ffmpeg_args();
        assert_eq!(args, vec!["-vf", "yadif=1"]);
    }

    #[test]
    fn test_filter_chain_multiple() {
        let mut chain = FilterChain::new();
        chain.add_filter("yadif=1".to_string());
        chain.add_filter("hqdn3d=1:1:2:2".to_string());
        chain.add_filter("scale=1920:1080".to_string());
        
        let args = chain.build_ffmpeg_args();
        assert_eq!(args, vec!["-vf", "yadif=1,hqdn3d=1:1:2:2,scale=1920:1080"]);
    }

    #[test]
    fn test_filter_builder_denoise() {
        let config = create_test_config();
        let chain = FilterBuilder::new(&config, None, None)
            .with_denoise(true)
            .build();
        
        let args = chain.build_ffmpeg_args();
        assert_eq!(args, vec!["-vf", "hqdn3d=1:1:2:2"]);
    }

    #[test]
    fn test_filter_builder_crop() {
        let config = create_test_config();
        let chain = FilterBuilder::new(&config, None, None)
            .with_crop(Some("1920:800:0:140"))
            .unwrap()
            .build();
        
        let args = chain.build_ffmpeg_args();
        assert_eq!(args, vec!["-vf", "crop=1920:800:0:140"]);
    }

    #[test]
    fn test_filter_builder_scale() {
        let config = create_test_config();
        let chain = FilterBuilder::new(&config, None, None)
            .with_scale(Some("1280x720"))
            .unwrap()
            .build();
        
        let args = chain.build_ffmpeg_args();
        assert_eq!(args, vec!["-vf", "scale=1280:720:flags=lanczos"]);
    }

    #[test]
    fn test_filter_builder_invalid_crop() {
        let config = create_test_config();
        let result = FilterBuilder::new(&config, None, None)
            .with_crop(Some("invalid"));
        
        assert!(result.is_err());
    }

    #[test]
    fn test_filter_builder_invalid_scale() {
        let config = create_test_config();
        let result = FilterBuilder::new(&config, None, None)
            .with_scale(Some("invalid"));
        
        assert!(result.is_err());
    }
}