use serde::{Deserialize, Serialize};
use std::path::Path;
use tracing::{debug, info, warn};

use crate::config::DolbyVisionConfig;
use crate::utils::{FfmpegWrapper, Result, Error};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DolbyVisionProfile {
    None,           // Not Dolby Vision
    Profile5,       // Single-layer DV only
    Profile7,       // Dual-layer (BL + EL + RPU)
    Profile81,      // Single-layer with HDR10 compatibility
    Profile82,      // Single-layer with SDR compatibility
    Profile84,      // HDMI streaming profile
}

impl DolbyVisionProfile {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Profile5 => "5",
            Self::Profile7 => "7",
            Self::Profile81 => "8.1",
            Self::Profile82 => "8.2",
            Self::Profile84 => "8.4",
        }
    }
    
    pub fn from_string(s: &str) -> Option<Self> {
        match s {
            "5" | "dvhe.05" => Some(Self::Profile5),
            "7" | "dvhe.07" => Some(Self::Profile7),
            "8.1" | "dvhe.08" | "dvhe.08.06" => Some(Self::Profile81),
            "8.2" | "dvhe.08.09" => Some(Self::Profile82),
            "8.4" | "dvhe.08.04" => Some(Self::Profile84),
            _ => None,
        }
    }
    
    pub fn supports_hdr10_compatibility(&self) -> bool {
        matches!(self, Self::Profile81 | Self::Profile84)
    }
    
    pub fn is_dual_layer(&self) -> bool {
        matches!(self, Self::Profile7)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DolbyVisionInfo {
    pub profile: DolbyVisionProfile,
    pub has_rpu: bool,
    pub has_enhancement_layer: bool,
    pub bl_compatible_id: Option<u8>,
    pub el_present: bool,
    pub rpu_present: bool,
    pub codec_profile: Option<String>,
    pub spatial_resampling_filter_hint: Option<u8>,
}

impl Default for DolbyVisionInfo {
    fn default() -> Self {
        Self::none()
    }
}

impl DolbyVisionInfo {
    pub fn none() -> Self {
        Self {
            profile: DolbyVisionProfile::None,
            has_rpu: false,
            has_enhancement_layer: false,
            bl_compatible_id: None,
            el_present: false,
            rpu_present: false,
            codec_profile: None,
            spatial_resampling_filter_hint: None,
        }
    }
    
    pub fn is_dolby_vision(&self) -> bool {
        self.profile != DolbyVisionProfile::None
    }
    
    pub fn needs_rpu_processing(&self) -> bool {
        self.has_rpu && self.profile != DolbyVisionProfile::None
    }
}


pub struct DolbyVisionDetector {
    config: DolbyVisionConfig,
}

impl DolbyVisionDetector {
    pub fn new(config: DolbyVisionConfig) -> Self {
        Self { config }
    }
    
    /// Analyze input file for Dolby Vision metadata
    pub async fn analyze<P: AsRef<Path>>(
        &self,
        ffmpeg: &FfmpegWrapper,
        input_path: P,
    ) -> Result<DolbyVisionInfo> {
        if !self.config.enabled {
            debug!("Dolby Vision detection disabled");
            return Ok(DolbyVisionInfo::none());
        }

        let path_str = input_path.as_ref().to_string_lossy();
        debug!("Analyzing Dolby Vision metadata for: {}", path_str);

        // Use ffprobe to extract detailed video stream information
        let output = ffmpeg.run_ffprobe(&[
            "-v", "quiet",
            "-select_streams", "v:0",
            "-show_entries", 
            "stream=codec_name,profile,codec_tag_string,color_space,color_transfer,color_primaries:stream_side_data",
            "-print_format", "json",
            &path_str,
        ]).await?;

        let json: serde_json::Value = serde_json::from_str(&output)
            .map_err(|e| Error::parse(format!("Failed to parse ffprobe output: {}", e)))?;

        self.parse_dolby_vision_metadata(&json)
    }
    
    fn parse_dolby_vision_metadata(&self, json: &serde_json::Value) -> Result<DolbyVisionInfo> {
        let streams = json["streams"].as_array()
            .ok_or_else(|| Error::parse("No streams found in ffprobe output"))?;
            
        if streams.is_empty() {
            return Ok(DolbyVisionInfo::none());
        }
        
        let stream = &streams[0];
        
        // Check codec and profile information
        let codec_name = stream["codec_name"].as_str().unwrap_or("");
        let profile = stream["profile"].as_str().unwrap_or("");
        let codec_tag = stream["codec_tag_string"].as_str().unwrap_or("");
        
        debug!("Stream info - codec: {}, profile: {}, tag: {}", codec_name, profile, codec_tag);
        
        // Look for Dolby Vision indicators
        let mut dv_info = self.detect_from_codec_info(codec_name, profile, codec_tag)?;
        
        // Check side data for additional Dolby Vision metadata
        if let Some(side_data_list) = stream["side_data_list"].as_array() {
            for side_data in side_data_list {
                if let Some(side_data_type) = side_data["side_data_type"].as_str() {
                    if side_data_type.contains("DOVI") || side_data_type.contains("dolby_vision") {
                        debug!("Found Dolby Vision side data: {}", side_data_type);
                        self.parse_side_data(&mut dv_info, side_data)?;
                    }
                }
            }
        }
        
        // Additional detection via color metadata
        if dv_info.profile == DolbyVisionProfile::None {
            dv_info = self.detect_from_color_metadata(stream)?;
        }
        
        if dv_info.is_dolby_vision() {
            info!("Detected Dolby Vision Profile {}: RPU={}, EL={}", 
                  dv_info.profile.as_str(), dv_info.has_rpu, dv_info.has_enhancement_layer);
        } else {
            debug!("No Dolby Vision metadata detected");
        }
        
        Ok(dv_info)
    }
    
    fn detect_from_codec_info(&self, codec_name: &str, profile: &str, codec_tag: &str) -> Result<DolbyVisionInfo> {
        let mut dv_info = DolbyVisionInfo::none();
        
        // Check for HEVC with Dolby Vision profile
        if codec_name == "hevc" || codec_name == "h265" {
            // Look for Dolby Vision profile in codec profile string
            if let Some(dv_profile) = self.extract_dolby_vision_profile(profile) {
                dv_info.profile = dv_profile;
                dv_info.codec_profile = Some(profile.to_string());
                
                // Set capabilities based on profile
                match dv_profile {
                    DolbyVisionProfile::Profile7 => {
                        dv_info.has_enhancement_layer = true;
                        dv_info.el_present = true;
                        dv_info.has_rpu = true;
                        dv_info.rpu_present = true;
                    },
                    DolbyVisionProfile::Profile81 | DolbyVisionProfile::Profile82 | DolbyVisionProfile::Profile84 => {
                        dv_info.has_rpu = true;
                        dv_info.rpu_present = true;
                        dv_info.has_enhancement_layer = false;
                        dv_info.el_present = false;
                    },
                    DolbyVisionProfile::Profile5 => {
                        dv_info.has_rpu = true;
                        dv_info.rpu_present = true;
                        dv_info.has_enhancement_layer = false;
                        dv_info.el_present = false;
                    },
                    _ => {}
                }
            }
        }
        
        // Check codec tag for Dolby Vision indicators
        if (codec_tag.contains("dvh") || codec_tag.contains("dvhe"))
            && dv_info.profile == DolbyVisionProfile::None {
                // Try to extract profile from codec tag
                if let Some(dv_profile) = DolbyVisionProfile::from_string(codec_tag) {
                    dv_info.profile = dv_profile;
                } else {
                    // Default to Profile 8.1 if we can't determine specific profile
                    dv_info.profile = DolbyVisionProfile::Profile81;
                }
                dv_info.has_rpu = true;
                dv_info.rpu_present = true;
            }
        
        Ok(dv_info)
    }
    
    fn extract_dolby_vision_profile(&self, profile_str: &str) -> Option<DolbyVisionProfile> {
        // Look for Dolby Vision profile patterns in the profile string
        if profile_str.contains("dvhe.05") {
            Some(DolbyVisionProfile::Profile5)
        } else if profile_str.contains("dvhe.07") {
            Some(DolbyVisionProfile::Profile7)
        } else if profile_str.contains("dvhe.08.06") || profile_str.contains("dvhe.08") {
            Some(DolbyVisionProfile::Profile81)
        } else if profile_str.contains("dvhe.08.09") {
            Some(DolbyVisionProfile::Profile82)
        } else if profile_str.contains("dvhe.08.04") {
            Some(DolbyVisionProfile::Profile84)
        } else {
            None
        }
    }
    
    fn parse_side_data(&self, dv_info: &mut DolbyVisionInfo, side_data: &serde_json::Value) -> Result<()> {
        // Parse Dolby Vision specific side data
        // First try numeric dv_profile (more common)
        if let Some(dv_profile_num) = side_data["dv_profile"].as_u64() {
            let profile = match dv_profile_num {
                5 => DolbyVisionProfile::Profile5,
                7 => DolbyVisionProfile::Profile7,
                8 => DolbyVisionProfile::Profile81, // Profile 8 typically maps to 8.1
                _ => {
                    debug!("Unknown Dolby Vision profile number: {}", dv_profile_num);
                    DolbyVisionProfile::None
                }
            };
            dv_info.profile = profile;
            debug!("Detected Dolby Vision profile {} from numeric value", dv_profile_num);
        }
        // Fallback to string parsing
        else if let Some(dv_profile_str) = side_data["dv_profile"].as_str() {
            if let Some(profile) = DolbyVisionProfile::from_string(dv_profile_str) {
                dv_info.profile = profile;
                debug!("Detected Dolby Vision profile from string: {}", dv_profile_str);
            }
        }
        
        if let Some(bl_compatible_id) = side_data["bl_compatible_id"].as_u64() {
            dv_info.bl_compatible_id = Some(bl_compatible_id as u8);
        }
        
        // Handle el_present_flag (can be bool or int)
        if let Some(el_present) = side_data["el_present_flag"].as_bool() {
            dv_info.el_present = el_present;
            dv_info.has_enhancement_layer = el_present;
            debug!("Enhancement Layer present flag (bool): {}", el_present);
        } else if let Some(el_present_int) = side_data["el_present_flag"].as_u64() {
            let el_present = el_present_int != 0;
            dv_info.el_present = el_present;
            dv_info.has_enhancement_layer = el_present;
            debug!("Enhancement Layer present flag (int): {} -> {}", el_present_int, el_present);
        }
        
        // Handle rpu_present_flag (can be bool or int)
        if let Some(rpu_present) = side_data["rpu_present_flag"].as_bool() {
            dv_info.rpu_present = rpu_present;
            dv_info.has_rpu = rpu_present;
            debug!("RPU present flag (bool): {}", rpu_present);
        } else if let Some(rpu_present_int) = side_data["rpu_present_flag"].as_u64() {
            let rpu_present = rpu_present_int != 0;
            dv_info.rpu_present = rpu_present;
            dv_info.has_rpu = rpu_present;
            debug!("RPU present flag (int): {} -> {}", rpu_present_int, rpu_present);
        }
        
        Ok(())
    }
    
    fn detect_from_color_metadata(&self, stream: &serde_json::Value) -> Result<DolbyVisionInfo> {
        let dv_info = DolbyVisionInfo::none();
        
        let color_space = stream["color_space"].as_str().unwrap_or("");
        let color_transfer = stream["color_transfer"].as_str().unwrap_or("");
        let color_primaries = stream["color_primaries"].as_str().unwrap_or("");
        
        // Heuristic detection: BT.2020 + SMPTE-2084 might be Dolby Vision
        // This is not foolproof but can catch some cases where codec metadata is missing
        if (color_space.contains("bt2020") || color_space.contains("rec2020")) &&
           color_transfer.contains("smpte2084") &&
           (color_primaries.contains("bt2020") || color_primaries.contains("rec2020")) {
            
            warn!("Detected HDR content that might be Dolby Vision based on color metadata, but no explicit DV markers found");
            // We don't set it as Dolby Vision since we can't be sure
            // The caller should handle this as HDR10 content
        }
        
        Ok(dv_info)
    }
    
    /// Determine if we should preserve Dolby Vision for this content
    pub fn should_preserve_dolby_vision(&self, dv_info: &DolbyVisionInfo) -> bool {
        if !self.config.enabled || !dv_info.is_dolby_vision() {
            return false;
        }
        
        match dv_info.profile {
            DolbyVisionProfile::None => false,
            DolbyVisionProfile::Profile7 => self.config.preserve_profile_7,
            _ => true, // Preserve other profiles by default
        }
    }
    
    /// Get the target profile for encoding
    pub fn get_target_profile(&self, source_profile: DolbyVisionProfile) -> DolbyVisionProfile {
        if !self.config.auto_profile_conversion {
            return source_profile;
        }
        
        match source_profile {
            DolbyVisionProfile::Profile7 => {
                // Convert Profile 7 to target profile (usually 8.1)
                match self.config.target_profile.as_str() {
                    "8.2" => DolbyVisionProfile::Profile82,
                    "8.4" => DolbyVisionProfile::Profile84,
                    _ => DolbyVisionProfile::Profile81, // Default to 8.1
                }
            },
            other => other, // Keep other profiles as-is
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_dolby_vision_profile_conversion() {
        assert_eq!(DolbyVisionProfile::from_string("dvhe.05"), Some(DolbyVisionProfile::Profile5));
        assert_eq!(DolbyVisionProfile::from_string("dvhe.07"), Some(DolbyVisionProfile::Profile7));
        assert_eq!(DolbyVisionProfile::from_string("dvhe.08.06"), Some(DolbyVisionProfile::Profile81));
        assert_eq!(DolbyVisionProfile::from_string("8.1"), Some(DolbyVisionProfile::Profile81));
        assert_eq!(DolbyVisionProfile::from_string("unknown"), None);
    }
    
    #[test]
    fn test_dolby_vision_info_defaults() {
        let info = DolbyVisionInfo::none();
        assert_eq!(info.profile, DolbyVisionProfile::None);
        assert!(!info.is_dolby_vision());
        assert!(!info.needs_rpu_processing());
    }
    
    #[test]
    fn test_profile_capabilities() {
        assert!(DolbyVisionProfile::Profile81.supports_hdr10_compatibility());
        assert!(DolbyVisionProfile::Profile7.is_dual_layer());
        assert!(!DolbyVisionProfile::Profile81.is_dual_layer());
    }
}