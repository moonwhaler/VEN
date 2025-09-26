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
    pub path: String,
    pub timeout_seconds: u64,
    pub extract_args: Option<Vec<String>>,
    pub inject_args: Option<Vec<String>>,
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
    pub dovi_tool: Option<DoviToolConfig>,
    pub hdr10plus_tool: Option<crate::hdr10plus::Hdr10PlusToolConfig>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LoggingConfig {
    pub level: String,
    pub show_timestamps: bool,
    pub colored_output: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CropDetectionConfig {
    pub enabled: bool,
    pub sample_count: u32,
    pub sdr_crop_limit: u32,
    pub hdr_crop_limit: u32,
    pub min_pixel_change_percent: f32,
}

impl Default for CropDetectionConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            sample_count: 3,
            sdr_crop_limit: 24,
            hdr_crop_limit: 64,
            min_pixel_change_percent: 1.0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToneMappingConfig {
    pub enabled: bool,
    pub target_max_nits: u32,
    pub algorithm: String, // "hable", "reinhard", "mobius", etc.
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UnifiedHdrConfig {
    pub enabled: bool,
    pub crf_adjustment: f32,
    pub bitrate_multiplier: f32,
    pub tone_mapping: Option<ToneMappingConfig>,
}

impl Default for UnifiedHdrConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            crf_adjustment: 2.0,
            bitrate_multiplier: 1.3,
            tone_mapping: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DolbyVisionConfig {
    pub enabled: bool,
    pub preserve_profile_7: bool,
    pub target_profile: String,
    pub require_dovi_tool: bool,
    pub temp_dir: Option<String>,
    pub auto_profile_conversion: bool,
    pub fallback_to_hdr10: bool,

    pub crf_adjustment: f32,
    pub bitrate_multiplier: f32,
    pub vbv_crf_bufsize: u32,
    pub vbv_crf_maxrate: u32,
    pub vbv_abr_bufsize: u32,
    pub vbv_abr_maxrate: u32,
    pub profile_specific_adjustments: bool,
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

            crf_adjustment: 1.0,
            bitrate_multiplier: 1.8,
            vbv_crf_bufsize: 80_000,
            vbv_crf_maxrate: 60_000,
            vbv_abr_bufsize: 120_000,
            vbv_abr_maxrate: 100_000,
            profile_specific_adjustments: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Hdr10PlusConfig {
    pub enabled: bool,

    pub temp_dir: Option<String>,

    pub require_tool: bool,

    pub fallback_to_hdr10: bool,

    pub crf_adjustment: f32,
    pub bitrate_multiplier: f32,
    pub encoding_complexity: f32,

    pub validate_curves: bool,
}

impl Default for Hdr10PlusConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            temp_dir: None,
            require_tool: false,
            fallback_to_hdr10: true,
            crf_adjustment: 2.5,
            bitrate_multiplier: 1.4,
            encoding_complexity: 1.4,
            validate_curves: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AnalysisConfig {
    pub crop_detection: CropDetectionConfig,
    pub hdr: Option<UnifiedHdrConfig>,
    pub dolby_vision: Option<DolbyVisionConfig>,
    pub hdr10_plus: Option<Hdr10PlusConfig>,
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct AudioSelectionConfig {
    #[serde(default)]
    pub languages: Option<Vec<String>>,
    #[serde(default)]
    pub codecs: Option<Vec<String>>,
    #[serde(default)]
    pub dispositions: Option<Vec<String>>,
    #[serde(default)]
    pub title_patterns: Option<Vec<String>>,
    #[serde(default)]
    pub exclude_commentary: bool,
    #[serde(default)]
    pub max_streams: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct SubtitleSelectionConfig {
    #[serde(default)]
    pub languages: Option<Vec<String>>,
    #[serde(default)]
    pub codecs: Option<Vec<String>>,
    #[serde(default)]
    pub dispositions: Option<Vec<String>>,
    #[serde(default)]
    pub title_patterns: Option<Vec<String>>,
    #[serde(default)]
    pub exclude_commentary: bool,
    #[serde(default)]
    pub include_forced_only: bool,
    #[serde(default)]
    pub max_streams: Option<usize>,
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
    pub bitrate: u32,
    pub content_type: String,
    pub x265_params: HashMap<String, serde_yaml::Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StreamSelectionProfile {
    pub name: String,
    pub title: String,
    pub audio: AudioSelectionConfig,
    pub subtitle: SubtitleSelectionConfig,
}

impl StreamSelectionProfile {
    pub fn from_raw(name: String, raw: RawStreamSelectionProfile) -> Self {
        Self {
            name,
            title: raw.title,
            audio: raw.audio.unwrap_or_default(),
            subtitle: raw.subtitle.unwrap_or_default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RawStreamSelectionProfile {
    pub title: String,
    pub audio: Option<AudioSelectionConfig>,
    pub subtitle: Option<SubtitleSelectionConfig>,
}
