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

    pub fn from_str(s: &str) -> Option<Self> {
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
    pub max_concurrent_jobs: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolsConfig {
    pub ffmpeg: String,
    pub ffprobe: String,
    pub nnedi_weights: Option<String>,
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
    pub stall_detection_seconds: u64,
    pub show_eta: bool,
    pub show_file_size: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ComplexityAnalysisConfig {
    pub enabled: bool,
    pub sample_points: Vec<f32>,
    pub methods: Vec<String>,
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HdrDetectionConfig {
    pub enabled: bool,
    pub color_space_patterns: Vec<String>,
    pub transfer_patterns: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AnalysisConfig {
    pub complexity_analysis: ComplexityAnalysisConfig,
    pub crop_detection: CropDetectionConfig,
    pub hdr_detection: HdrDetectionConfig,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WebSearchConfig {
    pub enabled: bool,
    pub timeout_seconds: u64,
    pub user_agent: String,
    pub simulation_mode: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ContentClassificationConfig {
    pub grain_thresholds: ThresholdConfig,
    pub motion_thresholds: ThresholdConfig,
    pub scene_change_thresholds: ThresholdConfig,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ThresholdConfig {
    pub low: u32,
    pub medium: u32,
    pub high: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ContentAdaptationConfig {
    pub crf_modifiers: HashMap<String, f32>,
    pub bitrate_multipliers: HashMap<String, f32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CudaConfig {
    pub enabled: bool,
    pub fallback_to_software: bool,
    pub decode_acceleration: bool,
    pub filter_acceleration: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HardwareConfig {
    pub cuda: CudaConfig,
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
    pub hardware_variant: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CropConfig {
    pub auto_detect: bool,
    pub validation: CropValidationConfig,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CropValidationConfig {
    pub min_change_percent: f32,
    pub temporal_samples: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScaleConfig {
    pub algorithm: String,
    pub preserve_aspect_ratio: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FiltersConfig {
    pub deinterlace: DeinterlaceConfig,
    pub denoise: DenoiseConfig,
    pub crop: CropConfig,
    pub scale: ScaleConfig,
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