use super::types::*;
use crate::config::UnifiedHdrConfig;
use crate::utils::{FfmpegWrapper, Result, Error};
use std::path::Path;
use tracing::{debug, warn};

pub struct HdrDetector {
    config: UnifiedHdrConfig,
}

impl HdrDetector {
    pub fn new(config: UnifiedHdrConfig) -> Self {
        Self { config }
    }

    pub async fn analyze<P: AsRef<Path>>(
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

        // Enhanced metadata extraction
        let enhanced_metadata = self.extract_enhanced_hdr_metadata(
            ffmpeg,
            &input_path,
        ).await?;

        let hdr_metadata = self.analyze_hdr_characteristics(&enhanced_metadata)?;
        let confidence = self.calculate_detection_confidence(&hdr_metadata);

        debug!("HDR Analysis Result: {:?} (confidence: {:.2})",
               hdr_metadata.format, confidence);

        Ok(HdrAnalysisResult {
            requires_tone_mapping: self.requires_tone_mapping(&hdr_metadata),
            encoding_complexity: self.calculate_encoding_complexity(&hdr_metadata),
            metadata: hdr_metadata,
            confidence_score: confidence,
        })
    }

    async fn extract_enhanced_hdr_metadata<P: AsRef<Path>>(
        &self,
        ffmpeg: &FfmpegWrapper,
        input_path: P,
    ) -> Result<EnhancedVideoMetadata> {
        // Enhanced ffprobe command to detect HDR10+ dynamic metadata
        let output = ffmpeg.run_ffprobe(&[
            "-v", "quiet",
            "-select_streams", "v:0",
            "-show_entries",
            "stream=color_space,color_transfer,color_primaries,bits_per_raw_sample,chroma_location:stream_side_data",
            "-show_frames",
            "-read_intervals", "%+#3", // Check first 3 frames for dynamic metadata
            "-print_format", "json",
            &input_path.as_ref().to_string_lossy(),
        ]).await?;

        let json: serde_json::Value = serde_json::from_str(&output)
            .map_err(|e| Error::parse(format!("Failed to parse HDR metadata: {}", e)))?;

        self.parse_enhanced_metadata(&json)
    }

    fn parse_enhanced_metadata(
        &self,
        json: &serde_json::Value,
    ) -> Result<EnhancedVideoMetadata> {
        let stream = json["streams"][0].as_object()
            .ok_or_else(|| Error::parse("No video stream found".to_string()))?;

        let color_space = stream.get("color_space")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let transfer_function = stream.get("color_transfer")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let color_primaries = stream.get("color_primaries")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let bit_depth = stream.get("bits_per_raw_sample")
            .and_then(|v| v.as_str())
            .and_then(|s| s.parse::<u8>().ok());

        let chroma_subsampling = stream.get("chroma_location")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        // Parse side data for HDR metadata
        let side_data = stream.get("side_data_list").and_then(|v| v.as_array());
        let mut master_display_metadata = None;
        let mut content_light_level = None;
        let mut has_dynamic_metadata = false;

        if let Some(side_data_array) = side_data {
            for side_data_entry in side_data_array {
                if let Some(side_data_type) = side_data_entry.get("side_data_type").and_then(|v| v.as_str()) {
                    match side_data_type {
                        "Mastering display metadata" => {
                            if let Some(md) = self.extract_mastering_display(side_data_entry) {
                                master_display_metadata = Some(md);
                            }
                        },
                        "Content light level metadata" => {
                            if let Some(cll) = self.extract_content_light_level(side_data_entry) {
                                content_light_level = Some(cll);
                            }
                        },
                        // Enhanced HDR10+ dynamic metadata detection
                        "HDR dynamic metadata (SMPTE 2094-40)" | 
                        "HDR dynamic metadata SMPTE2094-40" |
                        "HDR10+ dynamic metadata" |
                        "Dynamic HDR10+ metadata" => {
                            debug!("Found HDR10+ dynamic metadata: {}", side_data_type);
                            has_dynamic_metadata = true;
                        },
                        _ => {
                            // Pattern-based detection for HDR10+ variations
                            if self.is_hdr10plus_dynamic_metadata(side_data_type) {
                                debug!("Pattern-matched HDR10+ dynamic metadata: {}", side_data_type);
                                has_dynamic_metadata = true;
                            }
                        }
                    }
                }
            }
        }
        
        // Also check frame-level side data if available
        if !has_dynamic_metadata {
            if let Some(frames) = json.get("frames").and_then(|v| v.as_array()) {
                for frame in frames.iter().take(3) { // Check first 3 frames
                    if let Some(frame_side_data) = frame.get("side_data_list").and_then(|v| v.as_array()) {
                        for side_data_entry in frame_side_data {
                            if let Some(side_data_type) = side_data_entry.get("side_data_type").and_then(|v| v.as_str()) {
                                if self.is_hdr10plus_dynamic_metadata(side_data_type) {
                                    debug!("Found HDR10+ dynamic metadata in frame: {}", side_data_type);
                                    has_dynamic_metadata = true;
                                    break;
                                }
                            }
                        }
                        if has_dynamic_metadata { break; }
                    }
                }
            }
        }

        Ok(EnhancedVideoMetadata {
            color_space,
            transfer_function,
            color_primaries,
            master_display_metadata,
            content_light_level,
            has_dynamic_metadata,
            bit_depth,
            chroma_subsampling,
        })
    }

    /// Enhanced HDR10+ dynamic metadata detection with pattern matching
    fn is_hdr10plus_dynamic_metadata(&self, side_data_type: &str) -> bool {
        // Exact matches for known HDR10+ side data types
        if matches!(side_data_type, 
            "HDR dynamic metadata (SMPTE 2094-40)" |
            "HDR dynamic metadata SMPTE2094-40" |
            "HDR Dynamic Metadata SMPTE2094-40 (HDR10+)" |
            "HDR10+ dynamic metadata" |
            "Dynamic HDR10+ metadata" |
            "SMPTE2094-40" |
            "SMPTE 2094-40"
        ) {
            return true;
        }
        
        // Pattern-based detection (case insensitive)
        let lower = side_data_type.to_lowercase();
        
        // Check for SMPTE 2094-40 standard variations
        if lower.contains("smpte2094-40") || 
           lower.contains("smpte 2094-40") ||
           lower.contains("smpte2094_40") {
            return true;
        }
        
        // Check for HDR10+ specific patterns
        if lower.contains("hdr10+") && lower.contains("dynamic") {
            return true;
        }
        
        // Check for dynamic metadata with 2094 reference
        if lower.contains("dynamic") && lower.contains("metadata") && 
           (lower.contains("2094") || lower.contains("hdr10+")) {
            return true;
        }
        
        false
    }

    fn extract_mastering_display(&self, side_data: &serde_json::Value) -> Option<String> {
        // Extract mastering display data from ffprobe output
        // Format varies, but typically includes RGB primaries and white point
        if let Some(red_x) = side_data.get("red_x").and_then(|v| v.as_str()) {
            // Build master display string if all components are present
            let red_y = side_data.get("red_y").and_then(|v| v.as_str())?;
            let green_x = side_data.get("green_x").and_then(|v| v.as_str())?;
            let green_y = side_data.get("green_y").and_then(|v| v.as_str())?;
            let blue_x = side_data.get("blue_x").and_then(|v| v.as_str())?;
            let blue_y = side_data.get("blue_y").and_then(|v| v.as_str())?;
            let white_point_x = side_data.get("white_point_x").and_then(|v| v.as_str())?;
            let white_point_y = side_data.get("white_point_y").and_then(|v| v.as_str())?;
            let max_luminance = side_data.get("max_luminance").and_then(|v| v.as_str())?;
            let min_luminance = side_data.get("min_luminance").and_then(|v| v.as_str())?;

            Some(format!(
                "G({},{})B({},{})R({},{})WP({},{})L({},{})",
                green_x, green_y, blue_x, blue_y, red_x, red_y,
                white_point_x, white_point_y, max_luminance, min_luminance
            ))
        } else {
            None
        }
    }

    fn extract_content_light_level(&self, side_data: &serde_json::Value) -> Option<String> {
        if let (Some(max_cll), Some(max_fall)) = (
            side_data.get("max_content").and_then(|v| v.as_str()),
            side_data.get("max_average").and_then(|v| v.as_str())
        ) {
            Some(format!("{},{}", max_cll, max_fall))
        } else {
            None
        }
    }

    fn analyze_hdr_characteristics(
        &self,
        metadata: &EnhancedVideoMetadata
    ) -> Result<HdrMetadata> {
        let format = self.determine_hdr_format(metadata);
        let color_space = self.parse_color_space(&metadata.color_space);
        let transfer_function = self.parse_transfer_function(&metadata.transfer_function);
        let color_primaries = self.parse_color_primaries(&metadata.color_primaries);

        let master_display = metadata.master_display_metadata.as_ref()
            .and_then(|md| crate::hdr::metadata::HdrMetadataExtractor::parse_master_display(md).ok());

        let content_light_level = metadata.content_light_level.as_ref()
            .and_then(|cll| crate::hdr::metadata::HdrMetadataExtractor::parse_content_light_level(cll).ok());

        Ok(HdrMetadata {
            format,
            color_space,
            transfer_function,
            color_primaries,
            master_display,
            content_light_level,
            raw_color_space: metadata.color_space.clone(),
            raw_transfer: metadata.transfer_function.clone(),
            raw_primaries: metadata.color_primaries.clone(),
        })
    }

    fn determine_hdr_format(&self, metadata: &EnhancedVideoMetadata) -> HdrFormat {
        // Enhanced HDR format detection logic
        if let Some(ref transfer) = metadata.transfer_function {
            if transfer.contains("smpte2084") {
                // Check for HDR10+ dynamic metadata
                if metadata.has_dynamic_metadata {
                    return HdrFormat::HDR10Plus;
                }
                return HdrFormat::HDR10;
            } else if transfer.contains("arib-std-b67") {
                return HdrFormat::HLG;
            }
        }

        // Additional checks for color space and primaries
        if let (Some(ref cs), Some(ref cp)) = (&metadata.color_space, &metadata.color_primaries) {
            if (cs.contains("bt2020") || cs.contains("rec2020")) &&
               (cp.contains("bt2020") || cp.contains("rec2020")) {
                // Probably HDR but without clear transfer function
                warn!("HDR color space detected but unclear transfer function");
                return HdrFormat::HDR10;
            }
        }

        HdrFormat::None
    }

    fn parse_color_space(&self, raw: &Option<String>) -> ColorSpace {
        match raw.as_deref() {
            Some(cs) if cs.contains("bt2020") || cs.contains("rec2020") => ColorSpace::Bt2020,
            Some(cs) if cs.contains("bt709") || cs.contains("rec709") => ColorSpace::Bt709,
            Some(cs) if cs.contains("dci-p3") => ColorSpace::DciP3,
            Some(cs) if cs.contains("display-p3") => ColorSpace::DisplayP3,
            _ => ColorSpace::Bt709,
        }
    }

    fn parse_transfer_function(&self, raw: &Option<String>) -> TransferFunction {
        match raw.as_deref() {
            Some(tf) if tf.contains("smpte2084") => TransferFunction::Smpte2084,
            Some(tf) if tf.contains("arib-std-b67") => TransferFunction::AribStdB67,
            Some(tf) if tf.contains("bt2020-10") => TransferFunction::Bt2020_10,
            Some(tf) if tf.contains("bt2020-12") => TransferFunction::Bt2020_12,
            _ => TransferFunction::Bt709,
        }
    }

    fn parse_color_primaries(&self, raw: &Option<String>) -> ColorSpace {
        self.parse_color_space(raw)
    }

    fn calculate_detection_confidence(&self, metadata: &HdrMetadata) -> f32 {
        let mut confidence: f32 = 0.0;

        // Transfer function gives the strongest signal
        match metadata.transfer_function {
            TransferFunction::Smpte2084 => confidence += 0.8,
            TransferFunction::AribStdB67 => confidence += 0.8,
            TransferFunction::Bt2020_10 | TransferFunction::Bt2020_12 => confidence += 0.6,
            _ => {}
        }

        // Color space provides additional confirmation
        if metadata.color_space == ColorSpace::Bt2020 { 
            confidence += 0.2 
        }

        // Metadata presence increases confidence
        if metadata.master_display.is_some() {
            confidence += 0.15;
        }
        if metadata.content_light_level.is_some() {
            confidence += 0.15;
        }

        confidence.min(1.0)
    }

    fn requires_tone_mapping(&self, metadata: &HdrMetadata) -> bool {
        // For now, assume tone mapping is not required
        // This could be expanded based on display characteristics
        matches!(metadata.format, HdrFormat::HDR10 | HdrFormat::HDR10Plus | HdrFormat::HLG)
    }

    fn calculate_encoding_complexity(&self, metadata: &HdrMetadata) -> f32 {
        match metadata.format {
            HdrFormat::None => 1.0,
            HdrFormat::HDR10 => 1.2,
            HdrFormat::HDR10Plus => 1.4,
            HdrFormat::HLG => 1.15,
        }
    }
}