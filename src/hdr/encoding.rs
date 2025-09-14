use super::types::*;
use super::metadata::HdrMetadataExtractor;
use std::collections::HashMap;
use tracing::warn;

pub struct HdrEncodingParameterBuilder;

impl HdrEncodingParameterBuilder {
    /// Build x265 parameters for HDR content
    pub fn build_hdr_x265_params(
        &self,
        hdr_metadata: &HdrMetadata,
        base_params: &HashMap<String, String>,
    ) -> HashMap<String, String> {
        let mut params = base_params.clone();

        match hdr_metadata.format {
            HdrFormat::None => {
                // SDR content - ensure no HDR parameters
                self.ensure_sdr_params(&mut params);
            },
            HdrFormat::HDR10 => {
                self.add_hdr10_params(&mut params, hdr_metadata);
            },
            HdrFormat::HDR10Plus => {
                self.add_hdr10_plus_params(&mut params, hdr_metadata);
            },
            HdrFormat::HLG => {
                self.add_hlg_params(&mut params, hdr_metadata);
            },
        }

        params
    }

    fn add_hdr10_params(
        &self,
        params: &mut HashMap<String, String>,
        metadata: &HdrMetadata
    ) {
        // Color space parameters
        params.insert("colorprim".to_string(), "bt2020".to_string());
        params.insert("transfer".to_string(), "smpte2084".to_string());
        params.insert("colormatrix".to_string(), "bt2020nc".to_string());

        // Master display metadata
        if let Some(ref md) = metadata.master_display {
            let md_string = HdrMetadataExtractor::format_master_display_for_x265(md);
            params.insert("master-display".to_string(), md_string);
        } else {
            // Use default HDR10 mastering display if not present
            let default_md = HdrMetadata::hdr10_default().master_display.unwrap();
            let md_string = HdrMetadataExtractor::format_master_display_for_x265(&default_md);
            params.insert("master-display".to_string(), md_string);
        }

        // Content light level
        if let Some(ref cll) = metadata.content_light_level {
            let cll_string = HdrMetadataExtractor::format_content_light_level_for_x265(cll);
            params.insert("max-cll".to_string(), cll_string);
        } else {
            // Use default HDR10 content light level if not present
            let default_cll = HdrMetadata::hdr10_default().content_light_level.unwrap();
            let cll_string = HdrMetadataExtractor::format_content_light_level_for_x265(&default_cll);
            params.insert("max-cll".to_string(), cll_string);
        }

        // HDR optimization parameters
        params.insert("hdr".to_string(), "".to_string());
        params.insert("hdr-opt".to_string(), "".to_string());

        // Ensure appropriate bit depth for HDR
        params.entry("output-depth".to_string()).or_insert("10".to_string());

        // HDR-specific encoding optimizations
        self.add_hdr_encoding_optimizations(params);
    }

    fn add_hdr10_plus_params(
        &self,
        params: &mut HashMap<String, String>,
        metadata: &HdrMetadata
    ) {
        // Start with HDR10 base
        self.add_hdr10_params(params, metadata);

        // HDR10+ specific parameters (future implementation)
        // Note: x265 support for HDR10+ dynamic metadata is limited
        // params.insert("dhdr10-opt".to_string(), "".to_string());
        warn!("HDR10+ encoding support is limited in x265");

        // Enhanced optimizations for dynamic metadata
        params.insert("psy-rd".to_string(), "2.0".to_string());
        params.insert("psy-rdoq".to_string(), "1.0".to_string());
    }

    fn add_hlg_params(
        &self,
        params: &mut HashMap<String, String>,
        metadata: &HdrMetadata
    ) {
        // HLG (Hybrid Log-Gamma) parameters
        params.insert("colorprim".to_string(), "bt2020".to_string());
        params.insert("transfer".to_string(), "arib-std-b67".to_string());
        params.insert("colormatrix".to_string(), "bt2020nc".to_string());

        // HLG doesn't typically use mastering display metadata
        // But preserve it if present from the source
        if let Some(ref md) = metadata.master_display {
            let md_string = HdrMetadataExtractor::format_master_display_for_x265(md);
            params.insert("master-display".to_string(), md_string);
        }

        // Content light level for HLG (if available)
        if let Some(ref cll) = metadata.content_light_level {
            let cll_string = HdrMetadataExtractor::format_content_light_level_for_x265(cll);
            params.insert("max-cll".to_string(), cll_string);
        }

        // Ensure appropriate bit depth for HDR
        params.entry("output-depth".to_string()).or_insert("10".to_string());

        // HLG-specific optimizations
        self.add_hlg_encoding_optimizations(params);
    }

    fn ensure_sdr_params(&self, params: &mut HashMap<String, String>) {
        // Remove any HDR-specific parameters that might interfere
        params.remove("master-display");
        params.remove("max-cll");
        params.remove("hdr");
        params.remove("hdr-opt");
        params.remove("dhdr10-opt");

        // Ensure SDR color parameters
        params.insert("colorprim".to_string(), "bt709".to_string());
        params.insert("transfer".to_string(), "bt709".to_string());
        params.insert("colormatrix".to_string(), "bt709".to_string());

        // SDR typically uses 8-bit or 10-bit
        // Don't force 8-bit as some profiles may prefer 10-bit for quality
        params.entry("output-depth".to_string()).or_insert("8".to_string());
    }

    fn add_hdr_encoding_optimizations(&self, params: &mut HashMap<String, String>) {
        // HDR-specific encoding optimizations for better quality preservation
        
        // Psychovisual optimizations for HDR content
        params.entry("psy-rd".to_string()).or_insert("2.0".to_string());
        params.entry("psy-rdoq".to_string()).or_insert("1.0".to_string());

        // Rate-distortion optimization
        params.entry("rd".to_string()).or_insert("4".to_string());

        // Motion estimation optimizations for HDR
        params.entry("me".to_string()).or_insert("umh".to_string());
        params.entry("subme".to_string()).or_insert("3".to_string());

        // Quantization optimizations
        params.entry("aq-mode".to_string()).or_insert("3".to_string());
        params.entry("aq-strength".to_string()).or_insert("0.8".to_string());

        // HDR content often benefits from stronger deblocking
        params.entry("deblock".to_string()).or_insert("1,1".to_string());

        // SAO (Sample Adaptive Offset) is particularly useful for HDR
        params.entry("sao".to_string()).or_insert("".to_string());

        // Transform optimizations
        params.entry("rect".to_string()).or_insert("".to_string());
        params.entry("amp".to_string()).or_insert("".to_string());
    }

    fn add_hlg_encoding_optimizations(&self, params: &mut HashMap<String, String>) {
        // HLG-specific encoding optimizations
        
        // HLG has different perceptual characteristics than PQ
        params.entry("psy-rd".to_string()).or_insert("1.8".to_string());
        params.entry("psy-rdoq".to_string()).or_insert("0.8".to_string());

        // Rate-distortion optimization
        params.entry("rd".to_string()).or_insert("4".to_string());

        // Motion estimation optimizations
        params.entry("me".to_string()).or_insert("umh".to_string());
        params.entry("subme".to_string()).or_insert("3".to_string());

        // Adaptive quantization for HLG
        params.entry("aq-mode".to_string()).or_insert("2".to_string());
        params.entry("aq-strength".to_string()).or_insert("0.7".to_string());

        // Deblocking for HLG content
        params.entry("deblock".to_string()).or_insert("0,0".to_string());

        // SAO optimization for HLG
        params.entry("sao".to_string()).or_insert("".to_string());
    }

    /// Get recommended CRF adjustment for HDR content
    pub fn get_hdr_crf_adjustment(format: HdrFormat) -> f32 {
        match format {
            HdrFormat::None => 0.0,
            HdrFormat::HDR10 => 2.0,      // HDR10 needs higher CRF
            HdrFormat::HDR10Plus => 2.5,  // HDR10+ needs even higher CRF
            HdrFormat::HLG => 1.5,        // HLG needs moderate adjustment
        }
    }

    /// Get recommended bitrate multiplier for HDR content
    pub fn get_hdr_bitrate_multiplier(format: HdrFormat) -> f32 {
        match format {
            HdrFormat::None => 1.0,
            HdrFormat::HDR10 => 1.3,      // 30% increase for HDR10
            HdrFormat::HDR10Plus => 1.4,  // 40% increase for HDR10+
            HdrFormat::HLG => 1.2,        // 20% increase for HLG
        }
    }

    /// Validate HDR encoding parameters
    pub fn validate_hdr_encoding_params(
        params: &HashMap<String, String>,
        hdr_format: HdrFormat,
    ) -> Result<(), String> {
        match hdr_format {
            HdrFormat::None => {
                // SDR should not have HDR parameters
                if params.contains_key("master-display") || params.contains_key("max-cll") {
                    return Err("SDR encoding should not include HDR metadata parameters".to_string());
                }
            },
            HdrFormat::HDR10 | HdrFormat::HDR10Plus => {
                // Validate required HDR10 parameters
                if !params.contains_key("master-display") {
                    return Err("HDR10 encoding requires master-display parameter".to_string());
                }

                if let Some(colorprim) = params.get("colorprim") {
                    if colorprim != "bt2020" {
                        return Err("HDR10 encoding requires bt2020 color primaries".to_string());
                    }
                }

                if let Some(transfer) = params.get("transfer") {
                    if transfer != "smpte2084" {
                        return Err("HDR10 encoding requires smpte2084 transfer function".to_string());
                    }
                }

                // Validate bit depth for HDR
                if let Some(depth) = params.get("output-depth") {
                    if depth != "10" && depth != "12" {
                        return Err("HDR encoding requires at least 10-bit depth".to_string());
                    }
                }
            },
            HdrFormat::HLG => {
                // Validate HLG parameters
                if let Some(transfer) = params.get("transfer") {
                    if transfer != "arib-std-b67" {
                        return Err("HLG encoding requires arib-std-b67 transfer function".to_string());
                    }
                }

                if let Some(colorprim) = params.get("colorprim") {
                    if colorprim != "bt2020" {
                        return Err("HLG encoding requires bt2020 color primaries".to_string());
                    }
                }
            },
        }

        Ok(())
    }

    /// Format parameters for command line usage
    pub fn format_params_for_command_line(params: &HashMap<String, String>) -> String {
        let mut formatted_params = Vec::new();
        
        for (key, value) in params {
            if value.is_empty() {
                // Boolean parameters (no value)
                formatted_params.push(key.clone());
            } else {
                // Key-value parameters
                formatted_params.push(format!("{}={}", key, value));
            }
        }
        
        formatted_params.join(":")
    }
}