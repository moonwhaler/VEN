use super::{EncodingRecommendations, HdrFormatHandler};
use crate::hdr::metadata::HdrMetadataExtractor;
use crate::hdr::types::{ColorSpace, HdrFormat, HdrMetadata, TransferFunction};
use std::collections::HashMap;

pub struct HlgHandler;

impl HlgHandler {
    pub fn new() -> Self {
        Self
    }
}

impl HdrFormatHandler for HlgHandler {
    fn format(&self) -> HdrFormat {
        HdrFormat::HLG
    }

    fn build_encoding_params(
        &self,
        metadata: &HdrMetadata,
        base_params: &HashMap<String, String>,
    ) -> HashMap<String, String> {
        let mut params = base_params.clone();

        // Core HLG color parameters
        params.insert("colorprim".to_string(), "bt2020".to_string());
        params.insert("transfer".to_string(), "arib-std-b67".to_string());
        params.insert("colormatrix".to_string(), "bt2020nc".to_string());

        // HLG doesn't typically require mastering display metadata
        // but preserve it if present from the source
        if let Some(ref md) = metadata.master_display {
            let md_string = HdrMetadataExtractor::format_master_display_for_x265(md);
            params.insert("master-display".to_string(), md_string);
            tracing::debug!("Preserving mastering display metadata for HLG content");
        }

        // Content light level for HLG (if available)
        if let Some(ref cll) = metadata.content_light_level {
            let cll_string = HdrMetadataExtractor::format_content_light_level_for_x265(cll);
            params.insert("max-cll".to_string(), cll_string);
        }

        // Ensure appropriate bit depth for HLG
        params.insert("output-depth".to_string(), "10".to_string());

        // HLG-specific encoding optimizations
        self.add_hlg_optimizations(&mut params);

        params
    }

    fn validate_metadata(&self, metadata: &HdrMetadata) -> Result<(), String> {
        // Verify this is HLG format
        if metadata.format != HdrFormat::HLG {
            return Err(format!("Expected HLG format, got {:?}", metadata.format));
        }

        // Verify transfer function
        if metadata.transfer_function != TransferFunction::AribStdB67 {
            return Err("HLG requires ARIB STD-B67 transfer function".to_string());
        }

        // Verify color space
        if metadata.color_space != ColorSpace::Bt2020 {
            return Err("HLG requires BT.2020 color space".to_string());
        }

        // Validate mastering display if present (less strict than HDR10)
        if let Some(ref md) = metadata.master_display {
            // HLG typically uses lower peak luminance than HDR10
            if md.max_luminance < 50 || md.max_luminance > 4000 {
                tracing::warn!(
                    "HLG max luminance {} outside typical range [50, 4000] nits",
                    md.max_luminance
                );
            }

            if md.min_luminance < 0.0001 || md.min_luminance > 1.0 {
                return Err(format!(
                    "HLG min luminance {} out of valid range [0.0001, 1.0] nits",
                    md.min_luminance
                ));
            }

            // Validate chromaticity coordinates
            let coords = [
                md.red_primary.0,
                md.red_primary.1,
                md.green_primary.0,
                md.green_primary.1,
                md.blue_primary.0,
                md.blue_primary.1,
                md.white_point.0,
                md.white_point.1,
            ];

            for (i, coord) in coords.iter().enumerate() {
                if *coord < 0.0 || *coord > 1.0 {
                    return Err(format!(
                        "HLG chromaticity coordinate {} out of range [0.0, 1.0]: {}",
                        i, coord
                    ));
                }
            }
        }

        // Validate content light level if present
        if let Some(ref cll) = metadata.content_light_level {
            // HLG typically has lower content light levels
            if cll.max_cll == 0 || cll.max_cll > 4000 {
                tracing::warn!(
                    "HLG max CLL {} outside typical range [1, 4000] nits",
                    cll.max_cll
                );
            }

            if cll.max_fall == 0 || cll.max_fall > cll.max_cll {
                return Err(format!(
                    "HLG max FALL {} invalid (must be > 0 and <= max CLL {})",
                    cll.max_fall, cll.max_cll
                ));
            }
        }

        Ok(())
    }

    fn get_encoding_recommendations(&self) -> EncodingRecommendations {
        let mut special_params = HashMap::new();

        // HLG-specific encoding parameters (more conservative than HDR10)
        special_params.insert("psy-rd".to_string(), "1.8".to_string());
        special_params.insert("psy-rdoq".to_string(), "0.8".to_string());
        special_params.insert("aq-mode".to_string(), "2".to_string());
        special_params.insert("aq-strength".to_string(), "0.7".to_string());
        special_params.insert("deblock".to_string(), "0,0".to_string()); // Lighter deblocking

        EncodingRecommendations {
            crf_adjustment: 1.5,     // HLG needs moderate CRF adjustment
            bitrate_multiplier: 1.2, // 20% bitrate increase
            minimum_bit_depth: 10,
            recommended_preset: Some("medium".to_string()), // Balanced preset
            special_params,
        }
    }
}

impl HlgHandler {
    fn add_hlg_optimizations(&self, params: &mut HashMap<String, String>) {
        // HLG has different perceptual characteristics than PQ
        params
            .entry("psy-rd".to_string())
            .or_insert("1.8".to_string());
        params
            .entry("psy-rdoq".to_string())
            .or_insert("0.8".to_string());

        // Rate-distortion optimization (moderate for HLG)
        params.entry("rd".to_string()).or_insert("3".to_string());

        // Motion estimation optimized for HLG content
        params.entry("me".to_string()).or_insert("hex".to_string()); // Slightly faster than umh
        params.entry("subme".to_string()).or_insert("2".to_string());

        // Adaptive quantization for HLG (less aggressive than HDR10)
        params
            .entry("aq-mode".to_string())
            .or_insert("2".to_string());
        params
            .entry("aq-strength".to_string())
            .or_insert("0.7".to_string());

        // Lighter deblocking for HLG content (HLG is more forgiving)
        params
            .entry("deblock".to_string())
            .or_insert("0,0".to_string());

        // Sample Adaptive Offset (beneficial for HLG)
        params.entry("sao".to_string()).or_insert("".to_string());

        // Transform optimizations
        params.entry("rect".to_string()).or_insert("".to_string());

        // Rate control settings for HLG
        params
            .entry("rc-lookahead".to_string())
            .or_insert("20".to_string());
        params
            .entry("bframes".to_string())
            .or_insert("3".to_string());
        params
            .entry("b-adapt".to_string())
            .or_insert("1".to_string());

        // HLG-specific quality settings
        params
            .entry("nr-intra".to_string())
            .or_insert("0".to_string());
        params
            .entry("nr-inter".to_string())
            .or_insert("0".to_string());

        // Conservative analysis settings for HLG
        params
            .entry("strong-intra-smoothing".to_string())
            .or_insert("".to_string());

        // HLG benefits from weighted prediction
        params
            .entry("weightp".to_string())
            .or_insert("2".to_string());
        params
            .entry("weightb".to_string())
            .or_insert("".to_string());

        // Conservative GOP structure for HLG
        params
            .entry("keyint".to_string())
            .or_insert("250".to_string());
        params
            .entry("min-keyint".to_string())
            .or_insert("25".to_string());

        // HLG-specific quality optimizations
        params
            .entry("qcomp".to_string())
            .or_insert("0.6".to_string());
        params
            .entry("ip-ratio".to_string())
            .or_insert("1.4".to_string());
        params
            .entry("pb-ratio".to_string())
            .or_insert("1.3".to_string());

        // Disable noise reduction for clean HLG content
        params
            .entry("nr-intra".to_string())
            .or_insert("0".to_string());
        params
            .entry("nr-inter".to_string())
            .or_insert("0".to_string());

        // HLG works well with moderate complexity settings
        params
            .entry("max-merge".to_string())
            .or_insert("3".to_string());
        params
            .entry("early-skip".to_string())
            .or_insert("".to_string());
    }
}

impl Default for HlgHandler {
    fn default() -> Self {
        Self::new()
    }
}
