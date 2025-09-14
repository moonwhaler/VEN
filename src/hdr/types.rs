use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum HdrFormat {
    None,      // SDR content
    HDR10,     // Static HDR with SMPTE-2084 PQ
    HDR10Plus, // Dynamic HDR with ST 2094-40 metadata
    HLG,       // Hybrid Log-Gamma (broadcast HDR)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ColorSpace {
    Bt709,     // Standard definition / HD
    Bt2020,    // Ultra HD / HDR
    DciP3,     // Digital cinema
    DisplayP3, // Apple/consumer displays
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TransferFunction {
    Bt709,      // Standard gamma
    Smpte2084,  // PQ (Perceptual Quantizer) - HDR10
    AribStdB67, // HLG (Hybrid Log-Gamma)
    Bt2020_10,  // BT.2020 10-bit
    Bt2020_12,  // BT.2020 12-bit
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MasteringDisplayColorVolume {
    pub red_primary: (f32, f32), // x, y chromaticity coordinates
    pub green_primary: (f32, f32),
    pub blue_primary: (f32, f32),
    pub white_point: (f32, f32),
    pub max_luminance: u32, // nits
    pub min_luminance: f32, // nits (fractional)
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ContentLightLevelInfo {
    pub max_cll: u32,  // Maximum Content Light Level (nits)
    pub max_fall: u32, // Maximum Frame Average Light Level (nits)
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HdrMetadata {
    pub format: HdrFormat,
    pub color_space: ColorSpace,
    pub transfer_function: TransferFunction,
    pub color_primaries: ColorSpace,
    pub master_display: Option<MasteringDisplayColorVolume>,
    pub content_light_level: Option<ContentLightLevelInfo>,

    // Raw metadata for debugging/logging
    pub raw_color_space: Option<String>,
    pub raw_transfer: Option<String>,
    pub raw_primaries: Option<String>,
}

#[derive(Debug, Clone)]
pub struct HdrAnalysisResult {
    pub metadata: HdrMetadata,
    pub confidence_score: f32, // Detection confidence (0.0-1.0)
    pub requires_tone_mapping: bool,
    pub encoding_complexity: f32, // Complexity multiplier for encoding
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EnhancedVideoMetadata {
    pub color_space: Option<String>,
    pub transfer_function: Option<String>,
    pub color_primaries: Option<String>,
    pub master_display_metadata: Option<String>,
    pub content_light_level: Option<String>,
    pub has_dynamic_metadata: bool,
    pub bit_depth: Option<u8>,
    pub chroma_subsampling: Option<String>,
}

/// Default HDR metadata values for different formats
impl HdrMetadata {
    pub fn sdr_default() -> Self {
        Self {
            format: HdrFormat::None,
            color_space: ColorSpace::Bt709,
            transfer_function: TransferFunction::Bt709,
            color_primaries: ColorSpace::Bt709,
            master_display: None,
            content_light_level: None,
            raw_color_space: Some("bt709".to_string()),
            raw_transfer: Some("bt709".to_string()),
            raw_primaries: Some("bt709".to_string()),
        }
    }

    pub fn hdr10_default() -> Self {
        Self {
            format: HdrFormat::HDR10,
            color_space: ColorSpace::Bt2020,
            transfer_function: TransferFunction::Smpte2084,
            color_primaries: ColorSpace::Bt2020,
            master_display: Some(MasteringDisplayColorVolume {
                green_primary: (0.17, 0.797),
                blue_primary: (0.131, 0.046),
                red_primary: (0.708, 0.292),
                white_point: (0.3127, 0.329),
                max_luminance: 1000,
                min_luminance: 0.01,
            }),
            content_light_level: Some(ContentLightLevelInfo {
                max_cll: 1000,
                max_fall: 400,
            }),
            raw_color_space: Some("bt2020nc".to_string()),
            raw_transfer: Some("smpte2084".to_string()),
            raw_primaries: Some("bt2020".to_string()),
        }
    }
}
