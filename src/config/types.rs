use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContentType {
    Anime,
    ClassicAnime,
    #[serde(rename = "3d_animation")]
    Animation3D,
    Film,
    HeavyGrain,
    LightGrain,
    Action,
    CleanDigital,
    Mixed,
}

impl ContentType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Anime => "anime",
            Self::ClassicAnime => "classic_anime",
            Self::Animation3D => "3d_animation",
            Self::Film => "film",
            Self::HeavyGrain => "heavy_grain",
            Self::LightGrain => "light_grain",
            Self::Action => "action",
            Self::CleanDigital => "clean_digital",
            Self::Mixed => "mixed",
        }
    }

    pub fn from_string(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "anime" => Some(Self::Anime),
            "classic_anime" => Some(Self::ClassicAnime),
            "3d_animation" => Some(Self::Animation3D),
            "film" => Some(Self::Film),
            "heavy_grain" => Some(Self::HeavyGrain),
            "light_grain" => Some(Self::LightGrain),
            "action" => Some(Self::Action),
            "clean_digital" => Some(Self::CleanDigital),
            "mixed" => Some(Self::Mixed),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AppConfig {
    pub temp_dir: String,
    pub stats_prefix: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DoviToolConfig {
    pub path: String,                      // Path to dovi_tool binary
    pub timeout_seconds: u64,              // Tool operation timeout
    pub extract_args: Option<Vec<String>>, // Custom extraction arguments
    pub inject_args: Option<Vec<String>>,  // Custom injection arguments
}

impl Default for DoviToolConfig {
    fn default() -> Self {
        Self {
            path: "dovi_tool".to_string(),
            timeout_seconds: 300,
            extract_args: None,
            inject_args: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolsConfig {
    pub ffmpeg: String,
    pub ffprobe: String,
    pub nnedi_weights: Option<String>,
    pub dovi_tool: Option<DoviToolConfig>, // NEW: Dolby Vision tool
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LoggingConfig {
    pub level: String,
    pub show_timestamps: bool,
    pub colored_output: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProgressConfig {
    pub update_interval_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CropDetectionConfig {
    pub enabled: bool,
    /// Number of evenly distributed sample points across video duration
    pub sample_count: u32,
    pub sdr_crop_limit: u32,
    pub hdr_crop_limit: u32,
    pub min_pixel_change_percent: f32,
}

impl Default for CropDetectionConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            sample_count: 3, // Default to 3 evenly distributed samples
            sdr_crop_limit: 24,
            hdr_crop_limit: 64,
            min_pixel_change_percent: 1.0,
        }
    }
}

// Legacy HDR detection config - replaced by UnifiedHdrConfig in hdr module
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HdrDetectionConfig {
    pub enabled: bool,
    pub color_space_patterns: Vec<String>,
    pub transfer_patterns: Vec<String>,
    pub crf_adjustment: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToneMappingConfig {
    pub enabled: bool,
    pub target_max_nits: u32,
    pub algorithm: String,  // "hable", "reinhard", "mobius", etc.
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UnifiedHdrConfig {
    pub enabled: bool,
    pub auto_detect_format: bool,           // Auto-detect HDR10/HLG/etc
    pub preserve_metadata: bool,            // Preserve all HDR metadata
    pub fallback_to_sdr: bool,             // Fallback if HDR processing fails  
    pub encoding_optimization: bool,        // Use HDR-optimized encoding
    pub crf_adjustment: f32,               // CRF adjustment for HDR
    pub bitrate_multiplier: f32,           // Bitrate multiplier for HDR
    pub force_10bit: bool,                 // Force 10-bit output for HDR
    pub tone_mapping: Option<ToneMappingConfig>, // Future tone mapping
}

impl Default for UnifiedHdrConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            auto_detect_format: true,
            preserve_metadata: true,
            fallback_to_sdr: true,
            encoding_optimization: true,
            crf_adjustment: 2.0,
            bitrate_multiplier: 1.3,
            force_10bit: true,
            tone_mapping: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DolbyVisionConfig {
    pub enabled: bool,
    pub preserve_profile_7: bool,          // Convert P7 to P8.1
    pub target_profile: String,            // "8.1" or "8.2"  
    pub require_dovi_tool: bool,           // Fail if dovi_tool missing
    pub temp_dir: Option<String>,          // RPU temporary storage
    pub auto_profile_conversion: bool,     // Auto convert profiles for compatibility
    pub fallback_to_hdr10: bool,          // Fallback to HDR10 if DV processing fails
    
    // NEW: Dolby Vision-specific encoding adjustments
    pub crf_adjustment: f32,               // CRF adjustment for DV content (+0.5 to +1.0)
    pub bitrate_multiplier: f32,           // Bitrate multiplier for DV (1.5-2.0x)
    pub vbv_bufsize: u32,                  // VBV buffer size (mandatory for DV)
    pub vbv_maxrate: u32,                  // VBV max rate (mandatory for DV)
    pub profile_specific_adjustments: bool, // Different settings per profile
}

impl Default for DolbyVisionConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            preserve_profile_7: true,
            target_profile: "8.1".to_string(),
            require_dovi_tool: false,
            temp_dir: None,
            auto_profile_conversion: true,
            fallback_to_hdr10: true,
            
            // Dolby Vision encoding adjustments based on research
            crf_adjustment: 1.0,        // Lower than HDR10's +2.0, use +1.0 for DV
            bitrate_multiplier: 1.8,    // Higher than HDR10's 1.3x, use 1.8x for DV
            vbv_bufsize: 160000,        // Required for Level 5.1 High Tier DV
            vbv_maxrate: 160000,        // Mandatory VBV constraint for DV compliance
            profile_specific_adjustments: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AnalysisConfig {
    pub crop_detection: CropDetectionConfig,
    pub hdr_detection: HdrDetectionConfig,  // Legacy - kept for backward compatibility
    pub hdr: Option<UnifiedHdrConfig>,      // New unified HDR config
    pub dolby_vision: Option<DolbyVisionConfig>, // NEW: Dolby Vision configuration
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NnediSettings {
    pub field: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DeinterlaceConfig {
    pub primary_method: String,
    pub fallback_method: String,
    pub nnedi_settings: NnediSettings,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DenoiseConfig {
    pub filter: String,
    pub params: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FiltersConfig {
    pub deinterlace: DeinterlaceConfig,
    pub denoise: DenoiseConfig,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RawProfile {
    pub title: String,
    pub base_crf: f32,
    pub base_bitrate: u32,
    pub hdr_bitrate: u32,
    pub content_type: String,
    pub x265_params: HashMap<String, serde_yaml::Value>,
}
