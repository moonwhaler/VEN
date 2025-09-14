use super::{EncodingRecommendations, HdrFormatHandler};
use crate::hdr::metadata::HdrMetadataExtractor;
use crate::hdr::types::{ColorSpace, HdrFormat, HdrMetadata, TransferFunction};
use std::collections::HashMap;
use tracing::warn;

pub struct Hdr10PlusHandler;

impl Hdr10PlusHandler {
    pub fn new() -> Self {
        Self
    }
}

impl HdrFormatHandler for Hdr10PlusHandler {
    fn format(&self) -> HdrFormat {
        HdrFormat::HDR10Plus
    }

    fn build_encoding_params(
        &self,
        metadata: &HdrMetadata,
        base_params: &HashMap<String, String>,
    ) -> HashMap<String, String> {
        let mut params = base_params.clone();

        // HDR10+ inherits all HDR10 parameters
        self.add_hdr10_base_params(&mut params, metadata);

        // Add HDR10+ specific optimizations
        self.add_hdr10_plus_optimizations(&mut params);

        // Note: Dynamic metadata injection would require additional processing
        // that's typically handled by external tools or specialized encoders
        warn!(
            "HDR10+ dynamic metadata requires external processing - only static metadata applied"
        );

        params
    }

    fn validate_metadata(&self, metadata: &HdrMetadata) -> Result<(), String> {
        // Verify this is HDR10+ format
        if metadata.format != HdrFormat::HDR10Plus {
            return Err(format!("Expected HDR10+ format, got {:?}", metadata.format));
        }

        // HDR10+ uses the same base requirements as HDR10
        if metadata.transfer_function != TransferFunction::Smpte2084 {
            return Err("HDR10+ requires SMPTE-2084 (PQ) transfer function".to_string());
        }

        if metadata.color_space != ColorSpace::Bt2020 {
            return Err("HDR10+ requires BT.2020 color space".to_string());
        }

        // HDR10+ should have static mastering display metadata
        if metadata.master_display.is_none() {
            warn!("HDR10+ content missing static mastering display metadata");
        }

        // Validate mastering display if present (same as HDR10)
        if let Some(ref md) = metadata.master_display {
            if md.max_luminance < 100 || md.max_luminance > 10000 {
                return Err(format!(
                    "HDR10+ max luminance {} out of typical range [100, 10000] nits",
                    md.max_luminance
                ));
            }

            if md.min_luminance < 0.0001 || md.min_luminance > 1.0 {
                return Err(format!(
                    "HDR10+ min luminance {} out of typical range [0.0001, 1.0] nits",
                    md.min_luminance
                ));
            }
        }

        // Content light level validation
        if let Some(ref cll) = metadata.content_light_level {
            if cll.max_cll == 0 || cll.max_cll > 10000 {
                return Err(format!(
                    "HDR10+ max CLL {} out of typical range [1, 10000] nits",
                    cll.max_cll
                ));
            }

            if cll.max_fall == 0 || cll.max_fall > cll.max_cll {
                return Err(format!(
                    "HDR10+ max FALL {} invalid (must be > 0 and <= max CLL {})",
                    cll.max_fall, cll.max_cll
                ));
            }
        }

        Ok(())
    }

    fn get_encoding_recommendations(&self) -> EncodingRecommendations {
        let mut special_params = HashMap::new();

        // HDR10+ specific encoding parameters (more aggressive than HDR10)
        special_params.insert("psy-rd".to_string(), "2.2".to_string());
        special_params.insert("psy-rdoq".to_string(), "1.2".to_string());
        special_params.insert("aq-mode".to_string(), "3".to_string());
        special_params.insert("aq-strength".to_string(), "0.9".to_string());
        special_params.insert("deblock".to_string(), "1,1".to_string());
        special_params.insert("rc-lookahead".to_string(), "40".to_string());

        EncodingRecommendations {
            crf_adjustment: 2.5,     // HDR10+ needs higher CRF due to complexity
            bitrate_multiplier: 1.4, // 40% bitrate increase
            minimum_bit_depth: 10,
            recommended_preset: Some("slower".to_string()), // Slower preset for better quality
            special_params,
        }
    }
}

impl Hdr10PlusHandler {
    fn add_hdr10_base_params(&self, params: &mut HashMap<String, String>, metadata: &HdrMetadata) {
        // Core HDR10 color parameters (same as HDR10)
        params.insert("colorprim".to_string(), "bt2020".to_string());
        params.insert("transfer".to_string(), "smpte2084".to_string());
        params.insert("colormatrix".to_string(), "bt2020nc".to_string());

        // Master display metadata
        if let Some(ref md) = metadata.master_display {
            let md_string = HdrMetadataExtractor::format_master_display_for_x265(md);
            params.insert("master-display".to_string(), md_string);
        } else {
            // Use standard HDR10 defaults
            let default_md = HdrMetadata::hdr10_default().master_display.unwrap();
            let md_string = HdrMetadataExtractor::format_master_display_for_x265(&default_md);
            params.insert("master-display".to_string(), md_string);
        }

        // Content light level information
        if let Some(ref cll) = metadata.content_light_level {
            let cll_string = HdrMetadataExtractor::format_content_light_level_for_x265(cll);
            params.insert("max-cll".to_string(), cll_string);
        } else {
            params.insert("max-cll".to_string(), "1000,400".to_string());
        }

        // HDR optimization flags
        params.insert("hdr".to_string(), "".to_string());
        params.insert("hdr-opt".to_string(), "".to_string());

        // Force 10-bit output
        params.insert("output-depth".to_string(), "10".to_string());
    }

    fn add_hdr10_plus_optimizations(&self, params: &mut HashMap<String, String>) {
        // Enhanced psychovisual optimizations for HDR10+
        params
            .entry("psy-rd".to_string())
            .or_insert("2.2".to_string());
        params
            .entry("psy-rdoq".to_string())
            .or_insert("1.2".to_string());

        // More aggressive rate-distortion optimization
        params.entry("rd".to_string()).or_insert("4".to_string());

        // Enhanced motion estimation for dynamic content
        params.entry("me".to_string()).or_insert("umh".to_string());
        params.entry("subme".to_string()).or_insert("4".to_string()); // Higher subme for better quality

        // Stronger adaptive quantization
        params
            .entry("aq-mode".to_string())
            .or_insert("3".to_string());
        params
            .entry("aq-strength".to_string())
            .or_insert("0.9".to_string());

        // Deblocking filter optimization
        params
            .entry("deblock".to_string())
            .or_insert("1,1".to_string());

        // Sample Adaptive Offset
        params.entry("sao".to_string()).or_insert("".to_string());

        // Transform optimizations
        params.entry("rect".to_string()).or_insert("".to_string());
        params.entry("amp".to_string()).or_insert("".to_string());

        // Enhanced rate control for dynamic metadata
        params
            .entry("rc-lookahead".to_string())
            .or_insert("40".to_string()); // Longer lookahead
        params
            .entry("bframes".to_string())
            .or_insert("6".to_string()); // More B-frames
        params
            .entry("b-adapt".to_string())
            .or_insert("2".to_string());
        params
            .entry("b-pyramid".to_string())
            .or_insert("".to_string());

        // Quality optimizations for HDR10+
        params
            .entry("nr-intra".to_string())
            .or_insert("0".to_string());
        params
            .entry("nr-inter".to_string())
            .or_insert("0".to_string());

        // Enhanced analysis
        params
            .entry("strong-intra-smoothing".to_string())
            .or_insert("".to_string());
        params
            .entry("constrained-intra".to_string())
            .or_insert("".to_string());

        // Temporal optimizations for dynamic metadata
        params
            .entry("weightb".to_string())
            .or_insert("".to_string());
        params
            .entry("weightp".to_string())
            .or_insert("2".to_string());

        // HDR10+ specific quality preservation
        params.entry("cutree".to_string()).or_insert("".to_string());
        params
            .entry("no-open-gop".to_string())
            .or_insert("".to_string());

        // Future HDR10+ dynamic metadata support placeholder
        // Note: This would require external tools like hdr10plus_tool
        // params.insert("dhdr10-info".to_string(), "metadata.json".to_string());
    }
}

impl Default for Hdr10PlusHandler {
    fn default() -> Self {
        Self::new()
    }
}
