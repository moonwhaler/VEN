use crate::utils::{Error, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// HDR10+ dynamic metadata structure based on hdr10plus_tool JSON output
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct Hdr10PlusMetadata {
    /// JSON metadata info
    #[serde(rename = "JSONInfo")]
    pub json_info: JsonInfo,

    /// Scene-based metadata (per-frame or per-scene)
    pub scene_info: Vec<SceneMetadata>,

    /// Tool information
    pub tool_info: Option<ToolInfo>,
}

/// JSON metadata information
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct JsonInfo {
    /// HDR10+ profile (A, B, etc.)
    #[serde(rename = "HDR10plusProfile")]
    pub hdr10plus_profile: String,

    /// Version of the metadata format
    pub version: String,
}

/// Tool information
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct ToolInfo {
    /// Tool name
    pub tool: String,

    /// Tool version
    pub version: String,
}

/// Scene metadata from hdr10plus_tool
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct SceneMetadata {
    /// Scene ID
    pub scene_id: u32,

    /// Scene frame index (frame number within scene)
    pub scene_frame_index: u32,

    /// Sequence frame index (absolute frame number)
    pub sequence_frame_index: u32,

    /// Number of processing windows
    pub number_of_windows: u32,

    /// Target system display maximum luminance (nits)
    pub targeted_system_display_maximum_luminance: u32,

    /// Bezier curve data for tone mapping
    pub bezier_curve_data: BezierCurveData,

    /// Luminance parameters
    pub luminance_parameters: LuminanceParameters,
}

/// Bezier curve data for tone mapping
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct BezierCurveData {
    /// Knee point X coordinate
    pub knee_point_x: u32,

    /// Knee point Y coordinate
    pub knee_point_y: u32,

    /// Anchor points for bezier curve
    pub anchors: Vec<u32>,
}

/// Luminance parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct LuminanceParameters {
    /// Average RGB value
    #[serde(rename = "AverageRGB")]
    pub average_rgb: u32,

    /// Maximum Scene Content Light Level (MaxSCL) for R, G, B
    pub max_scl: Vec<u32>,

    /// Luminance distribution information
    pub luminance_distributions: Option<LuminanceDistributions>,
}

/// Luminance distribution data
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct LuminanceDistributions {
    /// Distribution percentile indices
    pub distribution_index: Vec<u32>,

    /// Distribution values for each index
    pub distribution_values: Vec<u32>,
}


/// HDR10+ metadata processing result
#[derive(Debug, Clone)]
pub struct Hdr10PlusProcessingResult {
    /// Path to extracted metadata JSON file
    pub metadata_file: PathBuf,

    /// Parsed metadata structure
    pub metadata: Hdr10PlusMetadata,

    /// Processing success flag
    pub extraction_successful: bool,

    /// File size of metadata
    pub file_size: Option<u64>,

    /// Number of tone mapping curves
    pub curve_count: u32,

    /// Scene count for Samsung compatibility
    pub scene_count: u32,
}

impl Hdr10PlusMetadata {
    /// Load HDR10+ metadata from JSON file
    pub async fn from_json_file<P: AsRef<std::path::Path>>(path: P) -> Result<Self> {
        let content = tokio::fs::read_to_string(&path).await.map_err(Error::Io)?;

        let metadata: Hdr10PlusMetadata =
            serde_json::from_str(&content).map_err(|e| Error::Parse {
                message: format!("Failed to parse HDR10+ metadata JSON: {}", e),
            })?;

        Ok(metadata)
    }

    /// Save HDR10+ metadata to JSON file
    pub async fn to_json_file<P: AsRef<std::path::Path>>(&self, path: P) -> Result<()> {
        let json = serde_json::to_string_pretty(self).map_err(|e| Error::Encoding {
            message: format!("Failed to serialize HDR10+ metadata: {}", e),
        })?;

        tokio::fs::write(&path, json).await.map_err(Error::Io)?;

        Ok(())
    }

    /// Get the number of frames with metadata
    pub fn get_frame_count(&self) -> u32 {
        self.scene_info.len() as u32
    }

    /// Get the number of unique scenes
    pub fn get_scene_count(&self) -> u32 {
        if self.scene_info.is_empty() {
            return 0;
        }
        self.scene_info
            .iter()
            .map(|s| s.scene_id)
            .max()
            .unwrap_or(0)
            + 1
    }

    /// Get the average brightness across all frames
    pub fn get_average_brightness(&self) -> Option<f64> {
        if self.scene_info.is_empty() {
            return None;
        }

        let sum: u32 = self
            .scene_info
            .iter()
            .map(|s| s.luminance_parameters.average_rgb)
            .sum();

        Some(sum as f64 / self.scene_info.len() as f64)
    }

    /// Get the peak brightness across all frames
    pub fn get_peak_brightness(&self) -> Option<u32> {
        self.scene_info
            .iter()
            .flat_map(|s| s.luminance_parameters.max_scl.iter())
            .max()
            .copied()
    }

    /// Check if metadata contains valid tone mapping curves
    pub fn has_tone_mapping_curves(&self) -> bool {
        self.scene_info
            .iter()
            .any(|s| !s.bezier_curve_data.anchors.is_empty())
    }

    /// Get the number of frames with tone mapping data
    pub fn get_tone_mapping_frame_count(&self) -> u32 {
        self.scene_info
            .iter()
            .filter(|s| !s.bezier_curve_data.anchors.is_empty())
            .count() as u32
    }

    /// Validate metadata consistency
    pub fn validate(&self) -> Result<()> {
        if self.scene_info.is_empty() {
            return Err(Error::Validation {
                message: "HDR10+ metadata contains no scene data".to_string(),
            });
        }

        // Validate that sequence frame indices are sequential
        for (i, scene) in self.scene_info.iter().enumerate() {
            if scene.sequence_frame_index != i as u32 {
                return Err(Error::Validation {
                    message: format!(
                        "Scene {} has sequence_frame_index {}, expected {}",
                        i, scene.sequence_frame_index, i
                    ),
                });
            }

            // Validate MaxSCL has 3 components (R, G, B)
            if scene.luminance_parameters.max_scl.len() != 3 {
                return Err(Error::Validation {
                    message: format!(
                        "Frame {} MaxSCL should have 3 components, found {}",
                        i,
                        scene.luminance_parameters.max_scl.len()
                    ),
                });
            }

            // Validate bezier curve has exactly 9 anchors
            let anchor_count = scene.bezier_curve_data.anchors.len();
            if anchor_count != 0 && anchor_count != 9 {
                return Err(Error::Validation {
                    message: format!(
                        "Frame {} bezier curve should have 0 or 9 anchors, found {}",
                        i, anchor_count
                    ),
                });
            }
        }

        Ok(())
    }
}

impl Default for Hdr10PlusMetadata {
    fn default() -> Self {
        Self {
            json_info: JsonInfo {
                hdr10plus_profile: "B".to_string(),
                version: "1.0".to_string(),
            },
            scene_info: Vec::new(),
            tool_info: None,
        }
    }
}

impl Hdr10PlusProcessingResult {
    /// Create a new processing result
    pub fn new(
        metadata_file: PathBuf,
        metadata: Hdr10PlusMetadata,
        extraction_successful: bool,
    ) -> Self {
        let curve_count = metadata.get_tone_mapping_frame_count();
        let scene_count = metadata.get_scene_count();

        Self {
            metadata_file,
            metadata,
            extraction_successful,
            file_size: None,
            curve_count,
            scene_count,
        }
    }

    /// Get the estimated processing overhead for HDR10+ content
    pub fn estimate_processing_overhead(&self) -> f32 {
        if !self.extraction_successful {
            return 1.0;
        }

        // Base overhead for HDR10+ processing
        let base_overhead = 1.4;

        // Additional overhead based on tone mapping complexity
        let curve_overhead = match self.curve_count {
            0 => 0.0,
            1..=100 => 0.1,
            101..=500 => 0.2,
            501..=1000 => 0.3,
            _ => 0.4, // Very complex tone mapping
        };

        // Additional overhead for scene changes
        let scene_overhead = (self.scene_count as f32 * 0.02).min(0.2);

        base_overhead + curve_overhead + scene_overhead
    }
}
