use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use crate::utils::{Result, Error};

/// HDR10+ dynamic metadata structure based on SMPTE ST 2094-40
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Hdr10PlusMetadata {
    /// Version of HDR10+ metadata format
    pub version: String,
    
    /// Number of frames with metadata
    pub num_frames: u32,
    
    /// Scene cuts information for Samsung compatibility
    pub scene_info: Option<Vec<SceneInfo>>,
    
    /// Per-frame tone mapping metadata
    pub frames: Vec<FrameMetadata>,
    
    /// Source file information
    pub source: Option<SourceInfo>,
}

/// Scene information for Samsung HDR10+ compatibility
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SceneInfo {
    /// Scene ID/number
    pub scene_id: u32,
    
    /// First frame index of this scene
    pub first_frame: u32,
    
    /// Last frame index of this scene
    pub last_frame: u32,
    
    /// Average brightness for the scene
    pub average_maxrgb: Option<f64>,
}

/// Per-frame HDR10+ dynamic metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FrameMetadata {
    /// Frame number (0-based)
    pub frame_index: u32,
    
    /// Target system display maximum luminance
    pub targeted_system_display_maximum_luminance: Option<f64>,
    
    /// Application version (usually 0 or 1)
    pub application_version: Option<u32>,
    
    /// Maximum of maxRGB values in the frame
    pub maxscl: Option<Vec<f64>>,
    
    /// Average of maxRGB values in the frame
    pub average_maxrgb: Option<f64>,
    
    /// Tone mapping parameters
    pub tone_mapping: Option<ToneMappingParams>,
    
    /// Color volume transform parameters
    pub color_volume_transform: Option<ColorVolumeTransform>,
}

/// Tone mapping parameters for HDR10+
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToneMappingParams {
    /// Knee point x coordinate (0-1 range)
    pub knee_point_x: f64,
    
    /// Knee point y coordinate (0-1 range)  
    pub knee_point_y: f64,
    
    /// Number of anchors in the tone mapping curve
    pub num_anchors: u32,
    
    /// Anchor points for tone mapping curve
    pub anchors: Vec<AnchorPoint>,
}

/// Anchor point for tone mapping curve
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnchorPoint {
    /// Input luminance value
    pub input: f64,
    
    /// Output luminance value
    pub output: f64,
}

/// Color volume transform parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColorVolumeTransform {
    /// Processing mode
    pub mode: u32,
    
    /// Color saturation gain
    pub saturation_gain: Option<f64>,
    
    /// Brightness adjustment
    pub brightness_adjustment: Option<f64>,
}

/// Source file information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceInfo {
    /// Original filename
    pub filename: Option<String>,
    
    /// File size in bytes
    pub file_size: Option<u64>,
    
    /// Creation timestamp
    pub created_at: Option<String>,
    
    /// Video resolution
    pub resolution: Option<String>,
    
    /// Frame rate
    pub frame_rate: Option<f64>,
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
        let content = tokio::fs::read_to_string(&path).await
            .map_err(|e| Error::Io(e))?;

        let metadata: Hdr10PlusMetadata = serde_json::from_str(&content)
            .map_err(|e| Error::Parse { message: format!(
                "Failed to parse HDR10+ metadata JSON: {}", e
            ) })?;

        Ok(metadata)
    }

    /// Save HDR10+ metadata to JSON file
    pub async fn to_json_file<P: AsRef<std::path::Path>>(&self, path: P) -> Result<()> {
        let json = serde_json::to_string_pretty(self)
            .map_err(|e| Error::Encoding { message: format!(
                "Failed to serialize HDR10+ metadata: {}", e
            ) })?;

        tokio::fs::write(&path, json).await
            .map_err(|e| Error::Io(e))?;

        Ok(())
    }

    /// Get the number of frames with tone mapping data
    pub fn get_tone_mapping_frame_count(&self) -> u32 {
        self.frames.iter()
            .filter(|frame| frame.tone_mapping.is_some())
            .count() as u32
    }

    /// Get the average brightness across all frames
    pub fn get_average_brightness(&self) -> Option<f64> {
        let brightness_values: Vec<f64> = self.frames.iter()
            .filter_map(|frame| frame.average_maxrgb)
            .collect();

        if brightness_values.is_empty() {
            None
        } else {
            Some(brightness_values.iter().sum::<f64>() / brightness_values.len() as f64)
        }
    }

    /// Get the peak brightness across all frames
    pub fn get_peak_brightness(&self) -> Option<f64> {
        self.frames.iter()
            .filter_map(|frame| frame.maxscl.as_ref())
            .flatten()
            .fold(None, |acc, &val| match acc {
                None => Some(val),
                Some(max) => Some(max.max(val)),
            })
    }

    /// Check if metadata contains valid tone mapping curves
    pub fn has_tone_mapping_curves(&self) -> bool {
        self.frames.iter().any(|frame| 
            frame.tone_mapping.is_some() && 
            frame.tone_mapping.as_ref().unwrap().anchors.len() > 0
        )
    }

    /// Get frame count with dynamic metadata
    pub fn get_dynamic_frame_count(&self) -> u32 {
        self.frames.iter()
            .filter(|frame| 
                frame.tone_mapping.is_some() || 
                frame.color_volume_transform.is_some()
            )
            .count() as u32
    }

    /// Validate metadata consistency
    pub fn validate(&self) -> Result<()> {
        if self.frames.is_empty() {
            return Err(Error::Validation { message:
                "HDR10+ metadata contains no frame data".to_string() });
        }

        if self.num_frames != self.frames.len() as u32 {
            return Err(Error::Validation { message:format!(
                "HDR10+ metadata frame count mismatch: declared {} but found {}",
                self.num_frames, self.frames.len()
            ) });
        }

        // Validate tone mapping curves
        for (i, frame) in self.frames.iter().enumerate() {
            if let Some(ref tm) = frame.tone_mapping {
                if tm.anchors.len() != tm.num_anchors as usize {
                    return Err(Error::Validation { message:format!(
                        "Frame {} tone mapping anchor count mismatch: declared {} but found {}",
                        i, tm.num_anchors, tm.anchors.len()
                    ) });
                }

                // Validate anchor points are in valid range
                for (j, anchor) in tm.anchors.iter().enumerate() {
                    if anchor.input < 0.0 || anchor.input > 1.0 {
                        return Err(Error::Validation { message: format!(
                            "Frame {} anchor {} input value {} out of range [0.0, 1.0]",
                            i, j, anchor.input
                        ) });
                    }
                    if anchor.output < 0.0 || anchor.output > 1.0 {
                        return Err(Error::Validation { message:format!(
                            "Frame {} anchor {} output value {} out of range [0.0, 1.0]",
                            i, j, anchor.output
                        ) });
                    }
                }
            }
        }

        Ok(())
    }
}

impl Default for Hdr10PlusMetadata {
    fn default() -> Self {
        Self {
            version: "1.0".to_string(),
            num_frames: 0,
            scene_info: None,
            frames: Vec::new(),
            source: None,
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
        let scene_count = metadata.scene_info.as_ref()
            .map(|scenes| scenes.len() as u32)
            .unwrap_or(0);

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