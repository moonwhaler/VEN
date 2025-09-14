pub mod detection;
pub mod encoding;
pub mod formats;
pub mod metadata;
pub mod types;

pub use detection::*;
pub use encoding::*;
pub use metadata::*;
pub use types::*;

use crate::config::UnifiedHdrConfig;
use crate::utils::{FfmpegWrapper, Result};
use std::path::Path;

/// High-level HDR manager that coordinates all HDR-related functionality
pub struct HdrManager {
    detector: HdrDetector,
    config: UnifiedHdrConfig,
}

impl HdrManager {
    pub fn new(config: UnifiedHdrConfig) -> Self {
        let detector = HdrDetector::new(config.clone());
        Self { detector, config }
    }

    /// Analyze content for HDR characteristics
    pub async fn analyze_content<P: AsRef<Path>>(
        &self,
        ffmpeg: &FfmpegWrapper,
        input_path: P,
    ) -> Result<HdrAnalysisResult> {
        if !self.config.enabled {
            return Ok(HdrAnalysisResult {
                metadata: HdrMetadata::sdr_default(),
                confidence_score: 1.0,
                requires_tone_mapping: false,
                encoding_complexity: 1.0,
            });
        }

        self.detector.analyze(ffmpeg, input_path).await
    }

    /// Get encoding complexity multiplier based on HDR characteristics
    pub fn get_encoding_complexity(&self, hdr_result: &HdrAnalysisResult) -> f32 {
        if !self.config.enabled {
            return 1.0;
        }

        match hdr_result.metadata.format {
            HdrFormat::None => 1.0,
            HdrFormat::HDR10 => 1.2,     // 20% complexity increase
            HdrFormat::HDR10Plus => 1.4, // 40% complexity increase
            HdrFormat::HLG => 1.15,      // 15% complexity increase
        }
    }

    /// Get CRF adjustment for HDR content
    pub fn get_crf_adjustment(&self, hdr_result: &HdrAnalysisResult) -> f32 {
        if !self.config.enabled {
            return 0.0;
        }

        match hdr_result.metadata.format {
            HdrFormat::None => 0.0,
            _ => self.config.crf_adjustment,
        }
    }

    /// Get bitrate multiplier for HDR content
    pub fn get_bitrate_multiplier(&self, hdr_result: &HdrAnalysisResult) -> f32 {
        if !self.config.enabled {
            return 1.0;
        }

        match hdr_result.metadata.format {
            HdrFormat::None => 1.0,
            _ => self.config.bitrate_multiplier,
        }
    }
}

// UnifiedHdrConfig is now defined in config/types.rs
