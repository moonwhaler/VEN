Modular HDR Implementation Redesign

  Based on my analysis of the current HDR implementation and the clean architecture established for Dolby Vision, here's a comprehensive redesign that creates a unified, 
  modular HDR/DV system.

  🎯 Current HDR Issues & Improvements

  Current Problems:

  1. Scattered HDR logic across multiple files (ffmpeg.rs, profiles.rs, main.rs)
  2. Hardcoded HDR metadata defaults instead of proper extraction
  3. Simple pattern matching for HDR detection
  4. Mixed responsibilities in parameter building
  5. No extensibility for different HDR formats (HDR10, HDR10+, HLG)

  🏗️ New Modular HDR Architecture

  1. Core HDR Module Structure

  src/
  ├── hdr/                        # NEW: Comprehensive HDR module
  │   ├── mod.rs                 # Module exports and unified interface
  │   ├── types.rs               # HDR type definitions
  │   ├── detection.rs           # HDR format detection
  │   ├── metadata.rs            # HDR metadata extraction and management
  │   ├── formats/               # HDR format implementations
  │   │   ├── mod.rs
  │   │   ├── hdr10.rs          # HDR10 implementation
  │   │   ├── hdr10_plus.rs     # HDR10+ support (future)
  │   │   └── hlg.rs            # HLG (Hybrid Log-Gamma) support
  │   └── encoding.rs           # HDR encoding parameter management
  ├── dolby_vision/              # DV module (from previous plan)
  │   └── ...                   # As designed previously
  └── color/                    # NEW: Unified color management
      ├── mod.rs
      ├── spaces.rs             # Color space definitions and conversions
      └── transfers.rs          # Transfer function management

  2. HDR Type System (src/hdr/types.rs)

  use serde::{Deserialize, Serialize};

  #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
  pub enum HdrFormat {
      None,        // SDR content
      HDR10,       // Static HDR with SMPTE-2084 PQ
      HDR10Plus,   // Dynamic HDR with ST 2094-40 metadata
      HLG,         // Hybrid Log-Gamma (broadcast HDR)
  }

  #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
  pub enum ColorSpace {
      Bt709,       // Standard definition / HD
      Bt2020,      // Ultra HD / HDR
      DciP3,       // Digital cinema
      DisplayP3,   // Apple/consumer displays
  }

  #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
  pub enum TransferFunction {
      Bt709,       // Standard gamma
      Smpte2084,   // PQ (Perceptual Quantizer) - HDR10
      AribStdB67,  // HLG (Hybrid Log-Gamma)
      Bt2020_10,   // BT.2020 10-bit
      Bt2020_12,   // BT.2020 12-bit
  }

  #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
  pub struct MasteringDisplayColorVolume {
      pub red_primary: (f32, f32),      // x, y chromaticity coordinates
      pub green_primary: (f32, f32),
      pub blue_primary: (f32, f32),
      pub white_point: (f32, f32),
      pub max_luminance: u32,            // nits
      pub min_luminance: f32,            // nits (fractional)
  }

  #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
  pub struct ContentLightLevelInfo {
      pub max_cll: u32,    // Maximum Content Light Level (nits)
      pub max_fall: u32,   // Maximum Frame Average Light Level (nits)
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
      pub confidence_score: f32,        // Detection confidence (0.0-1.0)
      pub requires_tone_mapping: bool,
      pub encoding_complexity: f32,     // Complexity multiplier for encoding
  }

  3. HDR Detection Engine (src/hdr/detection.rs)

  use super::types::*;
  use crate::utils::{FfmpegWrapper, VideoMetadata, Result, Error};
  use std::path::Path;
  use tracing::{debug, info, warn};

  pub struct HdrDetector {
      config: HdrDetectionConfig,
  }

  impl HdrDetector {
      pub fn new(config: HdrDetectionConfig) -> Self {
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
          let video_metadata = ffmpeg.get_video_metadata(&input_path).await?;
          let enhanced_metadata = self.extract_enhanced_hdr_metadata(
              ffmpeg,
              &input_path,
              &video_metadata
          ).await?;

          let hdr_metadata = self.analyze_hdr_characteristics(&enhanced_metadata)?;
          let confidence = self.calculate_detection_confidence(&hdr_metadata);

          debug!("HDR Analysis Result: {:?} (confidence: {:.2})",
                 hdr_metadata.format, confidence);

          Ok(HdrAnalysisResult {
              metadata: hdr_metadata,
              confidence_score: confidence,
              requires_tone_mapping: self.requires_tone_mapping(&hdr_metadata),
              encoding_complexity: self.calculate_encoding_complexity(&hdr_metadata),
          })
      }

      async fn extract_enhanced_hdr_metadata<P: AsRef<Path>>(
          &self,
          ffmpeg: &FfmpegWrapper,
          input_path: P,
          base_metadata: &VideoMetadata,
      ) -> Result<EnhancedVideoMetadata> {
          // Use ffprobe to get detailed HDR metadata
          let detailed_output = ffmpeg.run_ffprobe(&[
              "-v", "quiet",
              "-select_streams", "v:0",
              "-show_entries",
              "stream=color_space,color_transfer,color_primaries:stream_side_data=mastering_display_color_volume,content_light_level",
              "-print_format", "json",
              &input_path.as_ref().to_string_lossy(),
          ]).await?;

          let json: serde_json::Value = serde_json::from_str(&detailed_output)
              .map_err(|e| Error::parse(format!("Failed to parse HDR metadata: {}", e)))?;

          self.parse_enhanced_metadata(&json, base_metadata)
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
              .and_then(|md| self.parse_master_display_metadata(md));

          let content_light_level = metadata.content_light_level.as_ref()
              .and_then(|cll| self.parse_content_light_level(cll));

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
  }

  4. HDR Metadata Management (src/hdr/metadata.rs)

  use super::types::*;
  use crate::utils::{Result, Error};

  pub struct HdrMetadataExtractor;

  impl HdrMetadataExtractor {
      /// Parse raw master display metadata string into structured data
      pub fn parse_master_display(raw: &str) -> Result<MasteringDisplayColorVolume> {
          // Parse format: "G(x,y)B(x,y)R(x,y)WP(x,y)L(max,min)"
          let re = regex::Regex::new(
              r"G\(([0-9.]+),([0-9.]+)\)B\(([0-9.]+),([0-9.]+)\)R\(([0-9.]+),([0-9.]+)\)WP\(([0-9.]+),([0-9.]+)\)L\(([0-9.]+),([0-9.]+)\)"
          ).map_err(|e| Error::parse(format!("Regex compilation failed: {}", e)))?;

          if let Some(captures) = re.captures(raw) {
              Ok(MasteringDisplayColorVolume {
                  green_primary: (
                      captures[1].parse()?,
                      captures[2].parse()?,
                  ),
                  blue_primary: (
                      captures[3].parse()?,
                      captures[4].parse()?,
                  ),
                  red_primary: (
                      captures[5].parse()?,
                      captures[6].parse()?,
                  ),
                  white_point: (
                      captures[7].parse()?,
                      captures[8].parse()?,
                  ),
                  max_luminance: captures[9].parse()?,
                  min_luminance: captures[10].parse()?,
              })
          } else {
              Err(Error::parse("Invalid master display format"))
          }
      }

      /// Generate x265 master-display parameter string
      pub fn format_master_display_for_x265(md: &MasteringDisplayColorVolume) -> String {
          format!(
              "G({:.4},{:.4})B({:.4},{:.4})R({:.4},{:.4})WP({:.4},{:.4})L({},{:.4})",
              md.green_primary.0, md.green_primary.1,
              md.blue_primary.0, md.blue_primary.1,
              md.red_primary.0, md.red_primary.1,
              md.white_point.0, md.white_point.1,
              md.max_luminance, md.min_luminance
          )
      }

      /// Parse content light level from raw string
      pub fn parse_content_light_level(raw: &str) -> Result<ContentLightLevelInfo> {
          // Expected format: "1000,400" (max_cll, max_fall)
          let parts: Vec<&str> = raw.split(',').collect();
          if parts.len() >= 2 {
              Ok(ContentLightLevelInfo {
                  max_cll: parts[0].parse()?,
                  max_fall: parts[1].parse()?,
              })
          } else if parts.len() == 1 {
              // Only max_cll provided
              Ok(ContentLightLevelInfo {
                  max_cll: parts[0].parse()?,
                  max_fall: 400, // Default reasonable value
              })
          } else {
              Err(Error::parse("Invalid content light level format"))
          }
      }
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

  5. HDR Encoding Parameters (src/hdr/encoding.rs)

  use super::types::*;
  use std::collections::HashMap;

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
          }

          // Content light level
          if let Some(ref cll) = metadata.content_light_level {
              params.insert("max-cll".to_string(),
                           format!("{},{}", cll.max_cll, cll.max_fall));
          }

          // HDR optimization parameters
          params.insert("hdr".to_string(), "".to_string());
          params.insert("hdr-opt".to_string(), "".to_string());

          // Ensure appropriate bit depth for HDR
          params.entry("output-depth".to_string()).or_insert("10".to_string());
      }

      fn add_hdr10_plus_params(
          &self,
          params: &mut HashMap<String, String>,
          metadata: &HdrMetadata
      ) {
          // Start with HDR10 base
          self.add_hdr10_params(params, metadata);

          // HDR10+ specific parameters (future implementation)
          // params.insert("dhdr10-opt".to_string(), "".to_string());
          warn!("HDR10+ encoding not yet fully implemented");
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

          // HLG doesn't use mastering display metadata typically
          // But preserve it if present
          if let Some(ref md) = metadata.master_display {
              let md_string = HdrMetadataExtractor::format_master_display_for_x265(md);
              params.insert("master-display".to_string(), md_string);
          }
      }

      fn ensure_sdr_params(&self, params: &mut HashMap<String, String>) {
          // Remove any HDR-specific parameters that might interfere
          params.remove("master-display");
          params.remove("max-cll");
          params.remove("hdr");
          params.remove("hdr-opt");

          // Ensure SDR color parameters
          params.insert("colorprim".to_string(), "bt709".to_string());
          params.insert("transfer".to_string(), "bt709".to_string());
          params.insert("colormatrix".to_string(), "bt709".to_string());
      }
  }

  6. Unified Configuration (src/config/types.rs - Extended)

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

  #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
  pub struct ToneMappingConfig {
      pub enabled: bool,
      pub target_max_nits: u32,
      pub algorithm: String,  // "hable", "reinhard", "mobius", etc.
  }

  // Replace old HdrDetectionConfig with UnifiedHdrConfig
  #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
  pub struct ExtendedAnalysisConfig {
      pub crop_detection: CropDetectionConfig,
      pub hdr: UnifiedHdrConfig,              // NEW: Unified HDR config
      pub dolby_vision: DolbyVisionConfig,    // From DV plan
  }

  🔗 Integration with Dolby Vision

  1. Unified High Dynamic Range Manager

  // src/hdr/mod.rs
  pub struct HighDynamicRangeManager {
      hdr_detector: HdrDetector,
      dv_detector: DolbyVisionDetector,
      config: UnifiedHdrConfig,
  }

  impl HighDynamicRangeManager {
      pub async fn analyze_content<P: AsRef<Path>>(
          &self,
          ffmpeg: &FfmpegWrapper,
          input_path: P,
      ) -> Result<ContentAnalysisResult> {
          // First check for Dolby Vision
          let dv_result = self.dv_detector.analyze(ffmpeg, &input_path).await?;

          // Then analyze HDR characteristics
          let hdr_result = self.hdr_detector.analyze(ffmpeg, &input_path).await?;

          Ok(ContentAnalysisResult {
              hdr_analysis: hdr_result,
              dolby_vision: dv_result,
              recommended_approach: self.determine_encoding_approach(&hdr_result, &dv_result),
          })
      }

      fn determine_encoding_approach(
          &self,
          hdr: &HdrAnalysisResult,
          dv: &DolbyVisionInfo,
      ) -> EncodingApproach {
          if dv.profile != DolbyVisionProfile::None && self.config.preserve_dolby_vision {
              EncodingApproach::DolbyVision(dv.clone())
          } else if hdr.metadata.format != HdrFormat::None {
              EncodingApproach::HDR(hdr.clone())
          } else {
              EncodingApproach::SDR
          }
      }
  }

  #[derive(Debug, Clone)]
  pub enum EncodingApproach {
      SDR,
      HDR(HdrAnalysisResult),
      DolbyVision(DolbyVisionInfo),
  }

  2. Updated Profile Building

  // src/config/profiles.rs - Completely redesigned
  impl EncodingProfile {
      pub fn build_x265_params_unified(
          &self,
          mode_params: Option<&HashMap<String, String>>,
          content_analysis: &ContentAnalysisResult,
          rpu_metadata: Option<&RpuMetadata>,
      ) -> String {
          let mut params = self.build_base_params(mode_params);

          match &content_analysis.recommended_approach {
              EncodingApproach::SDR => {
                  // Ensure clean SDR parameters
                  let sdr_builder = HdrEncodingParameterBuilder;
                  params = sdr_builder.build_hdr_x265_params(
                      &HdrMetadata::sdr_default(),
                      &params
                  );
              },
              EncodingApproach::HDR(hdr_analysis) => {
                  // Add HDR-specific parameters
                  let hdr_builder = HdrEncodingParameterBuilder;
                  params = hdr_builder.build_hdr_x265_params(
                      &hdr_analysis.metadata,
                      &params
                  );
              },
              EncodingApproach::DolbyVision(dv_info) => {
                  // Add both HDR and DV parameters
                  let hdr_builder = HdrEncodingParameterBuilder;
                  let dv_builder = DolbyVisionParameterBuilder;

                  params = hdr_builder.build_hdr_x265_params(
                      &content_analysis.hdr_analysis.metadata,
                      &params
                  );

                  if let Some(rpu) = rpu_metadata {
                      params = dv_builder.add_dolby_vision_params(&params, rpu);
                  }
              },
          }

          self.format_params(params)
      }
  }

  🚀 Migration Benefits

  1. Clean Separation of Concerns

  - HDR detection: Isolated in dedicated detector
  - Metadata extraction: Comprehensive and structured
  - Parameter building: Format-specific builders
  - Integration: Unified manager for HDR/DV coordination

  2. Extensibility

  - New HDR formats: Easy to add HDR10+, future formats
  - Custom metadata: Structured approach for complex metadata
  - Different encoders: Not tied to x265 specifically

  3. Reliability

  - Proper error handling: Each component has comprehensive error handling
  - Fallback mechanisms: Graceful degradation when detection fails
  - Validation: Parameter validation at each stage

  4. Performance

  - Conditional processing: Skip expensive operations when not needed
  - Async throughout: Non-blocking metadata extraction
  - Caching: Metadata results can be cached for performance

  This redesigned HDR implementation provides a clean, modular foundation that integrates seamlessly with the Dolby Vision system while being extensible for future HDR
  technologies.