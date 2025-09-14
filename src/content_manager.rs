/// Unified content manager for HDR and Dolby Vision
///
/// This module provides a high-level interface that coordinates HDR and Dolby Vision
/// detection, analysis, and parameter adjustments based on the requirements documented
/// in "docs/Dolby Vision CRF Requirements.md"
use crate::analysis::dolby_vision::{DolbyVisionDetector, DolbyVisionInfo, DolbyVisionProfile};
use crate::config::{DolbyVisionConfig, Hdr10PlusConfig};
use crate::config::UnifiedHdrConfig;
use crate::hdr::{HdrManager, HdrAnalysisResult, HdrFormat};
use crate::hdr10plus::{Hdr10PlusManager, Hdr10PlusProcessingResult};
use crate::utils::{FfmpegWrapper, Result};
use std::path::Path;
use tracing::{debug, info, warn};

#[derive(Debug, Clone)]
pub struct ContentAnalysisResult {
    pub hdr_analysis: HdrAnalysisResult,
    pub dolby_vision: DolbyVisionInfo,
    pub hdr10_plus: Option<Hdr10PlusProcessingResult>,
    pub recommended_approach: ContentEncodingApproach,
    pub encoding_adjustments: EncodingAdjustments,
}

#[derive(Debug, Clone)]
pub enum ContentEncodingApproach {
    SDR,
    HDR(HdrAnalysisResult),
    DolbyVision(DolbyVisionInfo),
    DolbyVisionWithHDR10Plus(DolbyVisionInfo, HdrAnalysisResult),
}

#[derive(Debug, Clone)]
pub struct EncodingAdjustments {
    pub crf_adjustment: f32,
    pub bitrate_multiplier: f32,
    pub encoding_complexity: f32,
    pub requires_vbv: bool,
    pub vbv_bufsize: Option<u32>,
    pub vbv_maxrate: Option<u32>,
    pub recommended_crf_range: (f32, f32),
}

impl EncodingAdjustments {
    pub fn sdr_default() -> Self {
        Self {
            crf_adjustment: 0.0,
            bitrate_multiplier: 1.0,
            encoding_complexity: 1.0,
            requires_vbv: false,
            vbv_bufsize: None,
            vbv_maxrate: None,
            recommended_crf_range: (18.0, 28.0), // Standard SDR range
        }
    }
}

/// Unified content manager that handles HDR, Dolby Vision, and HDR10+ analysis
pub struct UnifiedContentManager {
    hdr_manager: HdrManager,
    dv_detector: Option<DolbyVisionDetector>,
    dv_config: Option<DolbyVisionConfig>,
    hdr10plus_manager: Option<Hdr10PlusManager>,
    _hdr10plus_config: Option<Hdr10PlusConfig>,
}

impl UnifiedContentManager {
    pub fn new(
        hdr_config: UnifiedHdrConfig, 
        dv_config: Option<DolbyVisionConfig>,
        _hdr10plus_config: Option<Hdr10PlusConfig>
    ) -> Self {
        let hdr_manager = HdrManager::new(hdr_config);
        let dv_detector = dv_config.as_ref()
            .filter(|config| config.enabled)
            .map(|config| DolbyVisionDetector::new(config.clone()));
        
        let hdr10plus_manager = _hdr10plus_config.as_ref()
            .filter(|config| config.enabled)
            .map(|config| {
                let temp_dir = config.temp_dir.as_deref()
                    .map(std::path::PathBuf::from)
                    .unwrap_or_else(|| std::path::PathBuf::from("/tmp"));
                Hdr10PlusManager::new(temp_dir, Some(config.tool.clone()))
            });
        
        Self {
            hdr_manager,
            dv_detector,
            dv_config,
            hdr10plus_manager,
            _hdr10plus_config,
        }
    }

    /// Analyze content for HDR, Dolby Vision, and HDR10+ characteristics
    pub async fn analyze_content<P: AsRef<Path>>(
        &self,
        ffmpeg: &FfmpegWrapper,
        input_path: P,
    ) -> Result<ContentAnalysisResult> {
        info!("Starting unified content analysis for: {}", input_path.as_ref().display());

        // First, analyze HDR characteristics
        let hdr_analysis = self.hdr_manager.analyze_content(ffmpeg, &input_path).await?;
        debug!("HDR analysis complete: format={:?}", hdr_analysis.metadata.format);

        // Then, check for Dolby Vision if enabled
        let dv_info = if let Some(ref detector) = self.dv_detector {
            detector.analyze(ffmpeg, &input_path).await?
        } else {
            DolbyVisionInfo::none()
        };
        debug!("Dolby Vision analysis complete: profile={:?}", dv_info.profile);

        // Process HDR10+ dynamic metadata if enabled and detected
        let hdr10plus_result = if let Some(ref manager) = self.hdr10plus_manager {
            // Try HDR10+ analysis for any HDR content (might be dual DV+HDR10+)
            if hdr_analysis.metadata.format == HdrFormat::HDR10Plus || 
               hdr_analysis.metadata.format == HdrFormat::HDR10 ||
               dv_info.is_dolby_vision() {
                info!("HDR10+ content detected - extracting dynamic metadata");
                if dv_info.is_dolby_vision() {
                    // Dual format processing
                    manager.process_dual_format(&input_path, &dv_info, &hdr_analysis).await?
                } else {
                    // HDR10+ only processing
                    manager.extract_hdr10plus_metadata(&input_path, &hdr_analysis).await?
                }
            } else {
                debug!("No HDR10+ content detected");
                None
            }
        } else {
            debug!("HDR10+ processing disabled");
            None
        };

        if let Some(ref result) = hdr10plus_result {
            info!("HDR10+ processing complete: {} frames with tone mapping",
                  result.curve_count);
        }

        // Determine the recommended encoding approach with all metadata
        let approach = self.determine_encoding_approach(&hdr_analysis, &dv_info, hdr10plus_result.as_ref());
        info!("Recommended encoding approach: {:?}", approach);

        // Calculate encoding adjustments based on the approach
        let adjustments = self.calculate_encoding_adjustments(&approach, &hdr_analysis, &dv_info);

        Ok(ContentAnalysisResult {
            hdr_analysis,
            dolby_vision: dv_info,
            hdr10_plus: hdr10plus_result,
            recommended_approach: approach,
            encoding_adjustments: adjustments,
        })
    }

    fn determine_encoding_approach(
        &self,
        hdr: &HdrAnalysisResult,
        dv: &DolbyVisionInfo,
        hdr10plus_result: Option<&Hdr10PlusProcessingResult>,
    ) -> ContentEncodingApproach {
        // Enhanced priority logic: DV+HDR10+ > DV > HDR10+ > HDR10 > SDR
        if dv.is_dolby_vision() {
            if let Some(ref config) = self.dv_config {
                if config.enabled {
                    if let Some(ref detector) = self.dv_detector {
                        if detector.should_preserve_dolby_vision(dv) {
                            // Check if we also have HDR10+ for dual encoding
                            // We check both the HDR format and if we successfully extracted HDR10+ metadata
                            let has_hdr10plus = hdr.metadata.format == HdrFormat::HDR10Plus || 
                                              hdr10plus_result.is_some();
                            
                            if has_hdr10plus {
                                info!("Dual format detected: Dolby Vision + HDR10+");
                                return ContentEncodingApproach::DolbyVisionWithHDR10Plus(
                                    dv.clone(), 
                                    hdr.clone()
                                );
                            }
                            return ContentEncodingApproach::DolbyVision(dv.clone());
                        }
                    }
                }
            }
            // Fallback: if DV can't be preserved, treat as HDR
            if hdr.metadata.format != HdrFormat::None {
                warn!("Dolby Vision detected but can't be preserved, falling back to HDR");
                return ContentEncodingApproach::HDR(hdr.clone());
            }
        }

        if hdr.metadata.format != HdrFormat::None {
            ContentEncodingApproach::HDR(hdr.clone())
        } else {
            ContentEncodingApproach::SDR
        }
    }

    fn calculate_encoding_adjustments(
        &self,
        approach: &ContentEncodingApproach,
        _hdr: &HdrAnalysisResult,
        _dv: &DolbyVisionInfo,
    ) -> EncodingAdjustments {
        match approach {
            ContentEncodingApproach::SDR => {
                EncodingAdjustments::sdr_default()
            },
            
            ContentEncodingApproach::HDR(hdr_result) => {
                // Standard HDR adjustments
                let crf_adjustment = self.hdr_manager.get_crf_adjustment(hdr_result);
                let bitrate_multiplier = self.hdr_manager.get_bitrate_multiplier(hdr_result);
                let encoding_complexity = self.hdr_manager.get_encoding_complexity(hdr_result);
                
                EncodingAdjustments {
                    crf_adjustment,
                    bitrate_multiplier,
                    encoding_complexity,
                    requires_vbv: false,
                    vbv_bufsize: None,
                    vbv_maxrate: None,
                    recommended_crf_range: (18.0, 24.0), // HDR range
                }
            },
            
            ContentEncodingApproach::DolbyVision(dv_info) => {
                // Dolby Vision-specific adjustments based on documentation
                if let Some(ref config) = self.dv_config {
                    let (crf_range, complexity_multiplier) = self.get_profile_specific_adjustments(dv_info, config);
                    
                    EncodingAdjustments {
                        crf_adjustment: config.crf_adjustment,
                        bitrate_multiplier: config.bitrate_multiplier,
                        encoding_complexity: complexity_multiplier,
                        requires_vbv: true,  // MANDATORY for Dolby Vision
                        vbv_bufsize: Some(config.vbv_bufsize),
                        vbv_maxrate: Some(config.vbv_maxrate),
                        recommended_crf_range: crf_range,
                    }
                } else {
                    // Fallback to conservative DV settings
                    EncodingAdjustments {
                        crf_adjustment: 1.0,
                        bitrate_multiplier: 1.8,
                        encoding_complexity: 1.5,
                        requires_vbv: true,
                        vbv_bufsize: Some(160000),
                        vbv_maxrate: Some(160000),
                        recommended_crf_range: (16.0, 20.0), // Conservative DV range
                    }
                }
            },
            
            ContentEncodingApproach::DolbyVisionWithHDR10Plus(dv_info, _hdr_result) => {
                // Dual format: even more conservative settings needed
                info!("Applying dual Dolby Vision + HDR10+ encoding adjustments");
                
                if let Some(ref config) = self.dv_config {
                    let (dv_crf_range, dv_complexity) = self.get_profile_specific_adjustments(dv_info, config);
                    
                    // Combine adjustments for both formats - very conservative
                    EncodingAdjustments {
                        crf_adjustment: config.crf_adjustment - 0.5, // Even lower CRF for dual format
                        bitrate_multiplier: config.bitrate_multiplier * 1.2, // 20% extra for HDR10+
                        encoding_complexity: dv_complexity * 1.3, // Higher complexity for dual metadata
                        requires_vbv: true,  // MANDATORY for both formats
                        vbv_bufsize: Some(config.vbv_bufsize),
                        vbv_maxrate: Some(config.vbv_maxrate),
                        recommended_crf_range: (dv_crf_range.0 - 1.0, dv_crf_range.1 - 0.5), // More conservative range
                    }
                } else {
                    // Fallback to very conservative dual format settings
                    EncodingAdjustments {
                        crf_adjustment: 0.5, // Very conservative CRF
                        bitrate_multiplier: 2.2, // High bitrate for dual metadata
                        encoding_complexity: 2.0, // High complexity
                        requires_vbv: true,
                        vbv_bufsize: Some(160000),
                        vbv_maxrate: Some(160000),
                        recommended_crf_range: (15.0, 18.0), // Very conservative range
                    }
                }
            }
        }
    }

    fn get_profile_specific_adjustments(
        &self,
        dv_info: &DolbyVisionInfo,
        config: &DolbyVisionConfig,
    ) -> ((f32, f32), f32) {
        if !config.profile_specific_adjustments {
            return ((16.0, 20.0), 1.5); // Default DV settings
        }

        match dv_info.profile {
            DolbyVisionProfile::Profile7 => {
                // Profile 7 (dual-layer) - more conservative encoding needed
                // Base layer quality is critical since EL will be discarded
                ((16.0, 19.0), 1.8) // Lower CRF range, higher complexity
            },
            
            DolbyVisionProfile::Profile81 => {
                // Profile 8.1 (single-layer with HDR10 compatibility)
                // Standard DV encoding range
                ((16.0, 20.0), 1.5)
            },
            
            DolbyVisionProfile::Profile82 => {
                // Profile 8.2 (single-layer with SDR compatibility)
                // Similar to 8.1 but may need slightly more conservative approach
                ((16.0, 19.0), 1.6)
            },
            
            DolbyVisionProfile::Profile84 => {
                // Profile 8.4 (HDMI streaming)
                // Similar requirements to 8.1
                ((16.0, 20.0), 1.5)
            },
            
            DolbyVisionProfile::Profile5 => {
                // Profile 5 (single-layer DV only, no HDR10 compatibility)
                // Can be slightly more aggressive since no compatibility constraints
                ((17.0, 21.0), 1.4)
            },
            
            _ => {
                // Fallback for unknown profiles
                warn!("Unknown Dolby Vision profile: {:?}, using conservative settings", dv_info.profile);
                ((16.0, 18.0), 1.8)
            }
        }
    }

    /// Get recommended CRF value for the given content
    pub fn get_recommended_crf(&self, result: &ContentAnalysisResult, base_crf: f32) -> f32 {
        let adjusted_crf = base_crf + result.encoding_adjustments.crf_adjustment;
        let (min_crf, max_crf) = result.encoding_adjustments.recommended_crf_range;
        
        // Clamp to the recommended range for the content type
        adjusted_crf.clamp(min_crf, max_crf)
    }

    /// Get recommended bitrate for the given content
    pub fn get_recommended_bitrate(&self, result: &ContentAnalysisResult, base_bitrate: u32) -> u32 {
        (base_bitrate as f32 * result.encoding_adjustments.bitrate_multiplier) as u32
    }

    /// Check if VBV constraints should be applied
    pub fn requires_vbv_constraints(&self, result: &ContentAnalysisResult) -> bool {
        result.encoding_adjustments.requires_vbv
    }

    /// Get VBV settings if required
    pub fn get_vbv_settings(&self, result: &ContentAnalysisResult) -> Option<(u32, u32)> {
        if result.encoding_adjustments.requires_vbv {
            Some((
                result.encoding_adjustments.vbv_bufsize.unwrap_or(160000),
                result.encoding_adjustments.vbv_maxrate.unwrap_or(160000)
            ))
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hdr::HdrMetadata;

    #[test]
    fn test_sdr_content_adjustments() {
        let hdr_config = UnifiedHdrConfig::default();
        let manager = UnifiedContentManager::new(hdr_config, None, None);
        
        let hdr_analysis = HdrAnalysisResult {
            metadata: HdrMetadata::sdr_default(),
            confidence_score: 1.0,
            requires_tone_mapping: false,
            encoding_complexity: 1.0,
        };
        
        let dv_info = DolbyVisionInfo::none();
        let approach = manager.determine_encoding_approach(&hdr_analysis, &dv_info, None);
        
        match approach {
            ContentEncodingApproach::SDR => {
                let adjustments = manager.calculate_encoding_adjustments(&approach, &hdr_analysis, &dv_info);
                assert_eq!(adjustments.crf_adjustment, 0.0);
                assert_eq!(adjustments.bitrate_multiplier, 1.0);
                assert!(!adjustments.requires_vbv);
            },
            _ => panic!("Expected SDR approach for SDR content"),
        }
    }

    #[test]
    fn test_dolby_vision_profile_specific_adjustments() {
        let hdr_config = UnifiedHdrConfig::default();
        let dv_config = DolbyVisionConfig::default();
        let manager = UnifiedContentManager::new(hdr_config, Some(dv_config.clone()), None);
        
        // Test Profile 7 (more conservative)
        let (crf_range_p7, complexity_p7) = manager.get_profile_specific_adjustments(
            &DolbyVisionInfo {
                profile: DolbyVisionProfile::Profile7,
                ..Default::default()
            },
            &dv_config
        );
        assert_eq!(crf_range_p7, (16.0, 19.0)); // More conservative range
        assert_eq!(complexity_p7, 1.8); // Higher complexity
        
        // Test Profile 8.1 (standard)
        let (crf_range_p81, complexity_p81) = manager.get_profile_specific_adjustments(
            &DolbyVisionInfo {
                profile: DolbyVisionProfile::Profile81,
                ..Default::default()
            },
            &dv_config
        );
        assert_eq!(crf_range_p81, (16.0, 20.0)); // Standard DV range
        assert_eq!(complexity_p81, 1.5); // Standard complexity
    }

    #[test]
    fn test_vbv_constraints_for_dolby_vision() {
        let hdr_config = UnifiedHdrConfig::default();
        let dv_config = DolbyVisionConfig::default();
        let manager = UnifiedContentManager::new(hdr_config, Some(dv_config), None);
        
        let dv_info = DolbyVisionInfo {
            profile: DolbyVisionProfile::Profile81,
            has_rpu: true,
            ..Default::default()
        };
        
        let hdr_analysis = HdrAnalysisResult {
            metadata: HdrMetadata::hdr10_default(),
            confidence_score: 1.0,
            requires_tone_mapping: false,
            encoding_complexity: 1.2,
        };
        
        let approach = manager.determine_encoding_approach(&hdr_analysis, &dv_info, None);
        let adjustments = manager.calculate_encoding_adjustments(&approach, &hdr_analysis, &dv_info);
        
        assert!(adjustments.requires_vbv);
        assert_eq!(adjustments.vbv_bufsize, Some(160000));
        assert_eq!(adjustments.vbv_maxrate, Some(160000));
        assert_eq!(adjustments.crf_adjustment, 1.0); // Lower than HDR's 2.0
        assert_eq!(adjustments.bitrate_multiplier, 1.8); // Higher than HDR's 1.3
    }
}