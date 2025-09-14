use super::{HdrFormatHandler, EncodingRecommendations};
use crate::hdr::types::{HdrFormat, HdrMetadata, TransferFunction, ColorSpace};
use crate::hdr::metadata::HdrMetadataExtractor;
use std::collections::HashMap;

pub struct Hdr10Handler;

impl Hdr10Handler {
    pub fn new() -> Self {
        Self
    }
}

impl HdrFormatHandler for Hdr10Handler {
    fn format(&self) -> HdrFormat {
        HdrFormat::HDR10
    }

    fn build_encoding_params(
        &self,
        metadata: &HdrMetadata,
        base_params: &HashMap<String, String>,
    ) -> HashMap<String, String> {
        let mut params = base_params.clone();

        // Core HDR10 color parameters
        params.insert("colorprim".to_string(), "bt2020".to_string());
        params.insert("transfer".to_string(), "smpte2084".to_string());
        params.insert("colormatrix".to_string(), "bt2020nc".to_string());

        // Master display metadata (required for HDR10)
        if let Some(ref md) = metadata.master_display {
            let md_string = HdrMetadataExtractor::format_master_display_for_x265(md);
            params.insert("master-display".to_string(), md_string);
        } else {
            // Use standard HDR10 defaults if not present
            let default_md = HdrMetadata::hdr10_default().master_display.unwrap();
            let md_string = HdrMetadataExtractor::format_master_display_for_x265(&default_md);
            params.insert("master-display".to_string(), md_string);
            tracing::warn!("Using default HDR10 mastering display metadata");
        }

        // Content light level information
        if let Some(ref cll) = metadata.content_light_level {
            let cll_string = HdrMetadataExtractor::format_content_light_level_for_x265(cll);
            params.insert("max-cll".to_string(), cll_string);
        } else {
            // Use conservative defaults if not present
            params.insert("max-cll".to_string(), "1000,400".to_string());
            tracing::warn!("Using default HDR10 content light level");
        }

        // HDR-specific optimization flags
        params.insert("hdr".to_string(), "".to_string());
        params.insert("hdr-opt".to_string(), "".to_string());

        // Force 10-bit output for HDR10
        params.insert("output-depth".to_string(), "10".to_string());

        // HDR10-specific encoding optimizations
        self.add_hdr10_optimizations(&mut params);

        params
    }

    fn validate_metadata(&self, metadata: &HdrMetadata) -> Result<(), String> {
        // Verify this is actually HDR10 format
        if metadata.format != HdrFormat::HDR10 {
            return Err(format!("Expected HDR10 format, got {:?}", metadata.format));
        }

        // Verify transfer function
        if metadata.transfer_function != TransferFunction::Smpte2084 {
            return Err("HDR10 requires SMPTE-2084 (PQ) transfer function".to_string());
        }

        // Verify color space
        if metadata.color_space != ColorSpace::Bt2020 {
            return Err("HDR10 requires BT.2020 color space".to_string());
        }

        // Validate mastering display if present
        if let Some(ref md) = metadata.master_display {
            // Check luminance range
            if md.max_luminance < 100 || md.max_luminance > 10000 {
                return Err(format!(
                    "HDR10 max luminance {} out of typical range [100, 10000] nits",
                    md.max_luminance
                ));
            }

            if md.min_luminance < 0.0001 || md.min_luminance > 1.0 {
                return Err(format!(
                    "HDR10 min luminance {} out of typical range [0.0001, 1.0] nits",
                    md.min_luminance
                ));
            }

            // Validate chromaticity coordinates are in valid range
            let coords = [
                md.red_primary.0, md.red_primary.1,
                md.green_primary.0, md.green_primary.1,
                md.blue_primary.0, md.blue_primary.1,
                md.white_point.0, md.white_point.1,
            ];

            for (i, coord) in coords.iter().enumerate() {
                if *coord < 0.0 || *coord > 1.0 {
                    return Err(format!(
                        "HDR10 chromaticity coordinate {} out of range [0.0, 1.0]: {}",
                        i, coord
                    ));
                }
            }
        }

        // Validate content light level if present
        if let Some(ref cll) = metadata.content_light_level {
            if cll.max_cll == 0 || cll.max_cll > 10000 {
                return Err(format!(
                    "HDR10 max CLL {} out of typical range [1, 10000] nits",
                    cll.max_cll
                ));
            }

            if cll.max_fall == 0 || cll.max_fall > cll.max_cll {
                return Err(format!(
                    "HDR10 max FALL {} invalid (must be > 0 and <= max CLL {})",
                    cll.max_fall, cll.max_cll
                ));
            }
        }

        Ok(())
    }

    fn get_encoding_recommendations(&self) -> EncodingRecommendations {
        let mut special_params = HashMap::new();
        
        // HDR10-specific encoding parameters
        special_params.insert("psy-rd".to_string(), "2.0".to_string());
        special_params.insert("psy-rdoq".to_string(), "1.0".to_string());
        special_params.insert("aq-mode".to_string(), "3".to_string());
        special_params.insert("aq-strength".to_string(), "0.8".to_string());
        special_params.insert("deblock".to_string(), "1,1".to_string());

        EncodingRecommendations {
            crf_adjustment: 2.0,  // HDR10 typically needs +2 CRF
            bitrate_multiplier: 1.3,  // 30% bitrate increase
            minimum_bit_depth: 10,
            recommended_preset: Some("slow".to_string()),
            special_params,
        }
    }
}

impl Hdr10Handler {
    fn add_hdr10_optimizations(&self, params: &mut HashMap<String, String>) {
        // Psychovisual optimizations for HDR10 content
        params.entry("psy-rd".to_string()).or_insert("2.0".to_string());
        params.entry("psy-rdoq".to_string()).or_insert("1.0".to_string());

        // Rate-distortion optimization
        params.entry("rd".to_string()).or_insert("4".to_string());

        // Motion estimation settings optimized for HDR
        params.entry("me".to_string()).or_insert("umh".to_string());
        params.entry("subme".to_string()).or_insert("3".to_string());

        // Adaptive quantization for HDR content
        params.entry("aq-mode".to_string()).or_insert("3".to_string());
        params.entry("aq-strength".to_string()).or_insert("0.8".to_string());

        // Deblocking filter - stronger for HDR content
        params.entry("deblock".to_string()).or_insert("1,1".to_string());

        // Sample Adaptive Offset
        params.entry("sao".to_string()).or_insert("".to_string());

        // Transform unit optimizations
        params.entry("rect".to_string()).or_insert("".to_string());
        params.entry("amp".to_string()).or_insert("".to_string());

        // Rate control optimizations for HDR
        params.entry("rc-lookahead".to_string()).or_insert("25".to_string());
        params.entry("bframes".to_string()).or_insert("4".to_string());
        params.entry("b-adapt".to_string()).or_insert("2".to_string());

        // HDR-specific quality optimizations
        params.entry("nr-intra".to_string()).or_insert("0".to_string()); // Disable noise reduction for HDR
        params.entry("nr-inter".to_string()).or_insert("0".to_string());
        
        // Strong analysis for HDR content
        params.entry("strong-intra-smoothing".to_string()).or_insert("".to_string());
        params.entry("constrained-intra".to_string()).or_insert("".to_string());
    }
}

impl Default for Hdr10Handler {
    fn default() -> Self {
        Self::new()
    }
}