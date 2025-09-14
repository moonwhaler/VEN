Dolby Vision Preservation Implementation Plan

  Based on your documentation and codebase analysis, here's a comprehensive plan to integrate Dolby Vision preservation using clean, modular architecture:

  🎯 Overview

  The current implementation correctly detects HDR content but strips Dolby Vision RPU metadata during re-encoding, converting DV content to static HDR10. This plan adds full
   Dolby Vision preservation with profile conversion support.

  🏗️ Proposed Architecture

  1. New Module Structure

  src/
  ├── analysis/
  │   ├── video.rs              # Existing HDR detection
  │   ├── dolby_vision.rs       # NEW: DV detection and analysis
  │   └── content.rs            # Existing content classification
  ├── dolby_vision/             # NEW: Comprehensive DV module
  │   ├── mod.rs               # Module exports
  │   ├── detection.rs         # DV profile detection
  │   ├── rpu.rs              # RPU extraction/injection
  │   ├── profiles.rs         # DV profile management
  │   └── tools.rs            # External tool integration
  ├── tools/                   # NEW: External tool wrappers
  │   ├── mod.rs
  │   └── dovi_tool.rs        # dovi_tool integration
  └── config/
      └── types.rs            # Extended with DV configuration

  2. Core Components

  A. Dolby Vision Detection (src/analysis/dolby_vision.rs)

  #[derive(Debug, Clone, PartialEq)]
  pub enum DolbyVisionProfile {
      None,           // Not Dolby Vision
      Profile5,       // Single-layer DV only
      Profile7,       // Dual-layer (BL + EL + RPU)
      Profile81,      // Single-layer with HDR10 compatibility
      Profile82,      // Single-layer with SDR compatibility
  }

  #[derive(Debug, Clone)]
  pub struct DolbyVisionInfo {
      pub profile: DolbyVisionProfile,
      pub has_rpu: bool,
      pub has_enhancement_layer: bool,
      pub bl_compatible_id: Option<u8>,
      pub el_present: bool,
      pub rpu_present: bool,
  }

  pub struct DolbyVisionDetector;

  impl DolbyVisionDetector {
      pub async fn analyze<P: AsRef<Path>>(
          &self,
          ffmpeg: &FfmpegWrapper,
          input_path: P
      ) -> Result<DolbyVisionInfo>;

      pub fn detect_profile(&self, metadata: &VideoMetadata) -> DolbyVisionProfile;
      pub fn should_preserve_dolby_vision(&self, dv_info: &DolbyVisionInfo) -> bool;
  }

  B. RPU Management (src/dolby_vision/rpu.rs)

  #[derive(Debug, Clone)]
  pub struct RpuMetadata {
      pub temp_file: PathBuf,
      pub profile: DolbyVisionProfile,
      pub frame_count: Option<u64>,
  }

  pub struct RpuManager {
      temp_dir: PathBuf,
      dovi_tool: DoviTool,
  }

  impl RpuManager {
      pub async fn extract_rpu<P: AsRef<Path>>(
          &self,
          input_path: P,
          dv_info: &DolbyVisionInfo,
      ) -> Result<Option<RpuMetadata>>;

      pub async fn inject_rpu<P: AsRef<Path>>(
          &self,
          encoded_path: P,
          rpu_metadata: &RpuMetadata,
      ) -> Result<PathBuf>;

      pub fn cleanup_rpu(&self, rpu_metadata: &RpuMetadata);
  }

  C. External Tool Integration (src/tools/dovi_tool.rs)

  pub struct DoviTool {
      path: String,
  }

  impl DoviTool {
      pub async fn extract_rpu<P: AsRef<Path>>(
          &self,
          input_path: P,
          output_rpu: P,
      ) -> Result<()>;

      pub async fn inject_rpu<P: AsRef<Path>>(
          &self,
          input_hevc: P,
          rpu_file: P,
          output_path: P,
      ) -> Result<()>;

      pub async fn check_availability(&self) -> Result<()>;
  }

  D. Configuration Extensions (src/config/types.rs)

  #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
  pub struct DolbyVisionConfig {
      pub enabled: bool,
      pub preserve_profile_7: bool,          // Convert P7 to P8.1
      pub target_profile: String,            // "8.1" or "8.2"  
      pub require_dovi_tool: bool,           // Fail if dovi_tool missing
      pub temp_dir: Option<String>,          // RPU temporary storage
  }

  #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
  pub struct DoviToolConfig {
      pub path: String,                      // Path to dovi_tool binary
      pub timeout_seconds: u64,              // Tool operation timeout
  }

  #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
  pub struct ExtendedToolsConfig {
      pub ffmpeg: String,
      pub ffprobe: String,
      pub nnedi_weights: Option<String>,
      pub dovi_tool: Option<DoviToolConfig>, // NEW: Dolby Vision tool
  }

  // Extend AnalysisConfig
  #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
  pub struct ExtendedAnalysisConfig {
      pub crop_detection: CropDetectionConfig,
      pub hdr_detection: HdrDetectionConfig,
      pub dolby_vision: DolbyVisionConfig,   // NEW: DV configuration
  }

  3. Integration Points

  A. Enhanced Video Analysis (src/analysis/video.rs)

  #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
  pub struct EnhancedVideoAnalysis {
      // Existing fields...
      pub is_hdr: bool,
      pub color_space: Option<String>,
      pub transfer_function: Option<String>,

      // NEW: Dolby Vision fields
      pub dolby_vision: Option<DolbyVisionInfo>,
      pub requires_rpu_processing: bool,
  }

  B. Extended Encoding Modes (src/encoding/modes.rs)

  impl Encoder for CrfEncoder {
      async fn encode<P: AsRef<Path>>(
          &self,
          // ... existing parameters
          dv_config: Option<&DolbyVisionConfig>,
          dv_info: Option<&DolbyVisionInfo>,
      ) -> Result<tokio::process::Child> {

          // NEW: Dolby Vision preprocessing
          let rpu_metadata = if let (Some(config), Some(dv_info)) = (dv_config, dv_info) {
              if config.enabled && dv_info.has_rpu {
                  Some(self.extract_rpu_metadata(input_path, dv_info).await?)
              } else { None }
          } else { None };

          // Build x265 parameters with DV support
          let x265_params = profile.build_x265_params_with_dolby_vision(
              Some(&mode_params),
              metadata,
              rpu_metadata.as_ref(),
          );

          // ... existing encoding logic

          // NEW: Post-process with RPU injection if needed
          if let Some(rpu) = rpu_metadata {
              return self.post_process_with_rpu(child, output_path, &rpu).await;
          }

          Ok(child)
      }
  }

  C. Profile Extension (src/config/profiles.rs)

  impl EncodingProfile {
      pub fn build_x265_params_with_dolby_vision(
          &self,
          mode_params: Option<&HashMap<String, String>>,
          metadata: &VideoMetadata,
          rpu_metadata: Option<&RpuMetadata>,
      ) -> String {
          let mut params = self.build_base_params(mode_params);

          // Add HDR parameters (existing)
          if metadata.is_hdr {
              self.add_hdr_parameters(&mut params, metadata);
          }

          // NEW: Add Dolby Vision parameters
          if let Some(rpu) = rpu_metadata {
              self.add_dolby_vision_parameters(&mut params, rpu);
          }

          self.format_params(params)
      }

      fn add_dolby_vision_parameters(
          &self,
          params: &mut HashMap<String, String>,
          rpu_metadata: &RpuMetadata,
      ) {
          params.insert("dolby-vision-rpu".to_string(),
                       rpu_metadata.temp_file.to_string_lossy().to_string());

          match rpu_metadata.profile {
              DolbyVisionProfile::Profile81 => {
                  params.insert("dolby-vision-profile".to_string(), "8.1".to_string());
              },
              DolbyVisionProfile::Profile82 => {
                  params.insert("dolby-vision-profile".to_string(), "8.2".to_string());
              },
              _ => {}
          }

          // Required VBV settings for Dolby Vision
          if !params.contains_key("vbv-bufsize") {
              params.insert("vbv-bufsize".to_string(), "20000".to_string());
          }
          if !params.contains_key("vbv-maxrate") {
              params.insert("vbv-maxrate".to_string(), "20000".to_string());
          }
      }
  }

  🔄 Workflow Integration

  1. Enhanced Processing Pipeline

  // In main.rs processing function
  async fn process_single_file_with_dv(/* params */) -> Result<()> {
      // 1. Existing analysis
      let metadata = ffmpeg.get_video_metadata(&input_path).await?;
      let video_analysis = analyze_video_content(&metadata)?;

      // 2. NEW: Dolby Vision analysis
      let dv_detector = DolbyVisionDetector::new(&config.analysis.dolby_vision);
      let dv_info = dv_detector.analyze(&ffmpeg, &input_path).await?;

      // 3. Profile selection (enhanced with DV awareness)
      let profile = profile_manager.select_profile_for_content(
          &video_analysis,
          Some(&dv_info)
      )?;

      // 4. Enhanced encoding with DV preservation
      let encoder = create_encoder(&args.mode);
      let child = encoder.encode(
          &ffmpeg,
          input_path,
          output_path,
          &profile,
          &filters,
          &stream_mapping,
          &metadata,
          adaptive_crf,
          adaptive_bitrate,
          custom_title,
          Some(&dv_info),  // NEW: Pass DV info
      ).await?;

      // 5. Monitor and handle completion
      monitor_encoding_with_dv_postprocessing(child, &dv_info).await?;
  }

  2. Configuration Schema Extension

  # config.yaml additions
  tools:
    ffmpeg: "/usr/bin/ffmpeg"
    ffprobe: "/usr/bin/ffprobe"
    dovi_tool:                          # NEW: Dolby Vision tool
      path: "/usr/local/bin/dovi_tool"
      timeout_seconds: 300

  analysis:
    # ... existing config
    dolby_vision:                       # NEW: DV configuration
      enabled: true
      preserve_profile_7: true          # Convert Profile 7 → 8.1
      target_profile: "8.1"             # Default target profile
      require_dovi_tool: false          # Optional dependency
      temp_dir: "/tmp"                  # RPU storage location

  profiles:
    # Profiles automatically enhanced with DV support
    film_4k:
      # ... existing parameters
      dolby_vision_compatible: true     # NEW: DV optimization flag

  🎯 Implementation Benefits

  1. Modular Architecture

  - Clean separation of concerns
  - Dolby Vision logic isolated in dedicated modules
  - Minimal changes to existing codebase
  - Easy to disable/enable DV support

  2. Profile Conversion Support

  - Profile 7 → 8.1: Automatic conversion for compatibility
  - Enhancement Layer handling: Graceful degradation when EL is discarded
  - VBV optimization: Proper buffer settings for DV content

  3. Error Handling & Fallback

  - Missing dovi_tool: Graceful fallback to HDR10 preservation
  - RPU extraction failure: Continue with standard HDR encoding
  - Temporary file cleanup: Robust resource management

  4. Performance Considerations

  - Conditional processing: DV analysis only when enabled
  - Async operations: Non-blocking RPU extraction/injection
  - Resource cleanup: Automatic temporary file management

  🚀 Phased Implementation

  Phase 1: Detection & Configuration

  1. Add DV configuration structures
  2. Implement basic DV detection in ffprobe analysis
  3. Add dovi_tool wrapper with availability checking

  Phase 2: RPU Management

  1. Implement RPU extraction logic
  2. Add temporary file management
  3. Create RPU injection post-processing

  Phase 3: Encoding Integration

  1. Extend x265 parameter building for DV
  2. Modify encoding modes to support DV workflow
  3. Add comprehensive error handling and fallback

  Phase 4: Testing & Optimization

  1. Add unit tests for DV detection and processing
  2. Performance optimization for large files
  3. Integration tests with real DV content

  This architecture provides clean, modular Dolby Vision support while maintaining the existing codebase's performance and reliability. The design allows for gradual rollout
  and easy maintenance.