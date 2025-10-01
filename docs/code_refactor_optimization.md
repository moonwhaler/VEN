# FFmpeg Autoencoder - Processing Module Refactoring Plan

## Executive Summary

This document provides a comprehensive analysis and optimization plan for the FFmpeg Autoencoder's `src/processing/mod.rs` file, which has grown to 612 lines and exhibits several architectural issues. The analysis reveals opportunities to reduce complexity by 67% while improving maintainability, testability, and user experience.

## Table of Contents

- [Current State Analysis](#current-state-analysis)
- [Architectural Issues Identified](#architectural-issues-identified)
- [Dependency Mapping](#dependency-mapping)
- [Code Duplication Analysis](#code-duplication-analysis)
- [Optimization Strategy](#optimization-strategy)
- [Implementation Phases](#implementation-phases)
- [Risk Assessment](#risk-assessment)
- [Expected Benefits](#expected-benefits)
- [Implementation Timeline](#implementation-timeline)

## Current State Analysis

### File Size Analysis
```
612 src/processing/mod.rs          # PRIMARY TARGET - Largest mod.rs
400 src/progress/mod.rs
151 src/color/mod.rs
 99 src/hdr/formats/mod.rs
 85 src/hdr/mod.rs
 11 src/utils/mod.rs
 10 src/analysis/mod.rs
  9 src/config/mod.rs
  7 src/hdr10plus/mod.rs
  7 src/encoding/mod.rs
  5 src/dolby_vision/mod.rs
  5 src/cli/mod.rs
  1 src/stream/mod.rs
```

The `processing/mod.rs` file is significantly larger than all other module files, indicating a concentration of responsibilities that should be distributed.

### VideoProcessor Structure

The `VideoProcessor` struct currently orchestrates:
- Content analysis coordination
- Profile selection and content classification
- Filter chain construction
- Stream mapping and preservation
- Encoding pipeline management
- Progress monitoring
- Metadata workflow management
- Logging and cleanup operations

## Architectural Issues Identified

### 1. God Object Pattern

**Problem**: The `VideoProcessor` handles too many responsibilities, violating the Single Responsibility Principle.

**Evidence**:
- **Method Count**: 15+ methods in a single implementation block
- **Dependency Count**: Direct dependencies on 13+ modules
- **Line Count**: 612 lines in a single file
- **Orchestration Complexity**: Manages lifecycles of multiple subsystems

**Impact**:
- Difficult to test individual components
- High coupling between unrelated functionalities
- Complex error propagation paths
- Hard to extend or modify specific features

### 2. Code Duplication

**Critical Duplication Found**:

**Location 1**: `src/processing/mod.rs:243-257`
```rust
async fn classify_content_from_metadata(
    &self,
    metadata: &VideoMetadata,
) -> Result<crate::config::ContentType> {
    use crate::config::ContentType;
    let bitrate_per_pixel =
        metadata.bitrate.unwrap_or(0) as f64 / (metadata.width as f64 * metadata.height as f64);
    if bitrate_per_pixel > 0.02 {
        Ok(ContentType::HeavyGrain)
    } else if bitrate_per_pixel > 0.015 {
        Ok(ContentType::LightGrain)
    } else {
        Ok(ContentType::Film)
    }
}
```

**Location 2**: `src/analysis/content.rs:24-44`
```rust
pub async fn classify_content(
    &self,
    metadata: &crate::utils::ffmpeg::VideoMetadata,
) -> Result<ContentClassification> {
    let bitrate_per_pixel = f64::from(metadata.bitrate.unwrap_or(0))
        / (f64::from(metadata.width) * f64::from(metadata.height));

    let content_type = if bitrate_per_pixel > 0.02 {
        ContentType::HeavyGrain
    } else if bitrate_per_pixel > 0.015 {
        ContentType::LightGrain
    } else {
        ContentType::Film
    };
    // ... rest of implementation
}
```

**Analysis**: Both implementations use identical heuristic logic but with different return types and error handling approaches.

### 3. Tight Coupling Issues

**Configuration Coupling**: Deep dependencies on nested configuration structures:
```rust
// Examples from VideoProcessor::new() and methods
self.config.analysis.hdr.clone().unwrap_or_default()
self.config.analysis.dolby_vision.clone()
self.config.tools.hdr10plus_tool.clone()
self.config.analysis.crop_detection
self.config.logging
self.config.progress
self.config.stream_selection_profiles
self.config.analysis.hdr_detection.passthrough_mode
```

**Component Coupling**: VideoProcessor directly instantiates and manages:
- `UnifiedContentManager`
- `MetadataWorkflowManager`
- `ProgressMonitor`
- `FileLogger`
- Multiple encoders (CRF, ABR, CBR)
- Stream mapping components

### 4. User Experience Issues

**FFmpeg Noise Problem** (as mentioned in CLAUDE.md):
Current output includes verbose diagnostic messages:
```
[matroska,webm @ 0x5555555f8780] Invalid Block Addition value 0x0 for unknown Block Addition Mapping type 68766345, name ""
[matroska,webm @ 0x5555555f8780] Could not find codec parameters for stream 3 (Subtitle: hdmv_pgs_subtitle (pgssub)): unspecified size
Consider increasing the value for the 'analyzeduration' (0) and 'probesize' (5000000) options
x265 [info]: HEVC encoder version 4.1
x265 [info]: build info [Linux][GCC 15.1.1][64 bit] 10bit
x265 [info]: using cpu capabilities: MMX2 SSE2Fast LZCNT SSSE3 SSE4.2 AVX FMA3 BMI2 AVX2
```

**Impact**: These messages clutter the output and provide no actionable information to users.

## Dependency Mapping

### Direct Dependencies (13+ modules)
```
VideoProcessor Dependencies:
├── cli::CliArgs                              # Command-line arguments
├── config::Config                            # Main configuration
├── config::EncodingProfile                   # Profile definitions
├── config::ProfileManager                    # Profile management
├── config::StreamSelectionProfileManager    # Stream profile management
├── encoding::modes::{Encoder, AbrEncoder, CbrEncoder, CrfEncoder}
├── encoding::{EncodingMode, FilterBuilder, FilterChain}
├── metadata_workflow::MetadataWorkflowManager
├── progress::ProgressMonitor
├── stream::preservation::StreamPreservation
├── utils::{ffmpeg::VideoMetadata, Error, FfmpegWrapper, FileLogger, Result}
├── ContentEncodingApproach
└── UnifiedContentManager
```

### Transitive Dependencies
Through `UnifiedContentManager`:
- `HdrManager`, `DolbyVisionDetector`, `Hdr10PlusManager`
- `analysis::{CropDetector, ContentAnalyzer}`
- `color::ColorManager`
- Various HDR, Dolby Vision, and HDR10+ utilities

### Dependency Graph Analysis
**No circular dependencies detected**, but **tight coupling patterns**:
- All components depend on `FfmpegWrapper`
- Configuration objects are deeply nested and widely shared
- Error types are coupled across module boundaries

## Code Duplication Analysis

### 1. Content Classification Logic
- **Duplication Factor**: 100% identical algorithm logic
- **Lines Duplicated**: ~15 lines
- **Maintenance Risk**: High - changes must be made in two places
- **Type Inconsistency**: Different return types between implementations

### 2. Parameter Building Patterns
- **x265 parameter construction**: Similar patterns across different encoding modes
- **Filter chain building**: Repeated validation logic
- **Progress message formatting**: Similar string construction patterns

### 3. Logging Patterns
- **File logging initialization**: Repeated across multiple methods
- **Progress reporting**: Similar message construction in multiple places
- **Error formatting**: Consistent patterns that could be centralized

## Optimization Strategy

The optimization follows a **4-phase incremental approach**, designed to minimize risk while maximizing benefits:

### Phase 1: Quick Wins (High Impact, Low Risk)
- Remove code duplication
- Add FFmpeg message filtering
- Improve user experience

### Phase 2: Extract Analysis Orchestration (Medium Risk)
- Create dedicated analysis coordinator
- Reduce VideoProcessor complexity
- Improve separation of concerns

### Phase 3: Extract Profile Selection (Medium Risk)
- Centralize profile selection logic
- Eliminate remaining duplication
- Improve configuration management

### Phase 4: Extract Encoding Pipeline (Higher Risk)
- Create dedicated encoding coordinator
- Transform VideoProcessor into lightweight orchestrator
- Complete architectural transformation

## Implementation Phases

### Phase 1: Quick Wins

#### 1.1 Remove Content Classification Duplication

**Objective**: Eliminate identical logic in two locations

**Changes**:
1. **Remove method** from `src/processing/mod.rs:243-257`:
   ```rust
   // DELETE this method entirely
   async fn classify_content_from_metadata(&self, metadata: &VideoMetadata) -> Result<ContentType>
   ```

2. **Update** `src/processing/mod.rs:218-241` to use existing service:
   ```rust
   async fn select_profile(&self, metadata: &VideoMetadata) -> Result<EncodingProfile> {
       if self.args.profile == "auto" {
           info!("Auto-selecting profile based on content analysis...");

           // Use existing ContentAnalyzer instead of duplicated logic
           let content_analyzer = ContentAnalyzer::new();
           let classification = content_analyzer.classify_content(metadata).await?;
           let content_type = classification.content_type;

           if let Some(profile) = self.profile_manager.recommend_profile_for_resolution(
               metadata.width,
               metadata.height,
               content_type,
           ) {
               Ok(profile.clone())
           } else {
               // ... rest unchanged
           }
       } else {
           // ... rest unchanged
       }
   }
   ```

3. **Add import** to `src/processing/mod.rs:1`:
   ```rust
   use crate::analysis::ContentAnalyzer;
   ```

**Impact**:
- **Lines Reduced**: 15
- **Duplication Eliminated**: 100%
- **Risk Level**: Very Low

#### 1.2 Add FFmpeg Message Filtering

**Objective**: Filter diagnostic messages that clutter user output

**Implementation**:

1. **Create message filter** in `src/utils/ffmpeg.rs`:
   ```rust
   /// Filter out FFmpeg diagnostic messages that don't provide user value
   fn filter_ffmpeg_stderr(stderr: &str) -> String {
       stderr
           .lines()
           .filter(|line| {
               // Filter out common diagnostic messages
               !line.contains("Invalid Block Addition") &&
               !line.contains("Could not find codec parameters") &&
               !line.contains("Consider increasing the value for") &&
               !line.contains("analyzeduration") &&
               !line.contains("probesize") &&
               // Keep error messages but filter info messages
               !(line.contains("x265 [info]:") && (
                   line.contains("encoder version") ||
                   line.contains("build info") ||
                   line.contains("using cpu capabilities") ||
                   line.contains("Thread pool created") ||
                   line.contains("Coding QT:") ||
                   line.contains("Residual QT:") ||
                   line.contains("ME / range") ||
                   line.contains("Keyframe min") ||
                   line.contains("Lookahead") ||
                   line.contains("b-pyramid") ||
                   line.contains("References") ||
                   line.contains("tools:")
               ))
           })
           .collect::<Vec<_>>()
           .join("\n")
   }
   ```

2. **Apply filtering** in relevant methods:
   ```rust
   // In get_video_metadata() method around line 106
   if !output.status.success() {
       let error_msg = filter_ffmpeg_stderr(&String::from_utf8_lossy(&output.stderr));
       return Err(Error::ffmpeg(format!("FFprobe failed: {}", error_msg)));
   }
   ```

3. **Optional**: Add configuration option for verbose mode:
   ```rust
   // In config/types.rs
   pub struct LoggingConfig {
       // ... existing fields
       pub show_ffmpeg_diagnostics: bool,
   }
   ```

**Impact**:
- **User Experience**: Significantly cleaner output
- **Risk Level**: Very Low
- **Backward Compatibility**: Maintained

### Phase 2: Extract Analysis Orchestration

#### 2.1 Create AnalysisOrchestrator

**Objective**: Extract analysis coordination logic from VideoProcessor

**New File**: `src/analysis/orchestrator.rs`
```rust
use crate::{
    analysis::{ContentAnalyzer, CropDetector},
    config::Config,
    utils::{ffmpeg::VideoMetadata, FfmpegWrapper, Result},
    UnifiedContentManager, ContentAnalysisResult,
};
use std::path::Path;

#[derive(Debug)]
pub struct CompleteAnalysisResult {
    pub content_analysis: ContentAnalysisResult,
    pub crop_values: Option<String>,
    pub crop_sample_timestamps: Vec<f64>,
    pub crop_analysis_result: Option<crate::analysis::CropAnalysisResult>,
}

pub struct AnalysisOrchestrator {
    content_manager: UnifiedContentManager,
    content_analyzer: ContentAnalyzer,
    crop_detector: Option<CropDetector>,
}

impl AnalysisOrchestrator {
    pub fn new(config: &Config) -> Result<Self> {
        let content_manager = UnifiedContentManager::new(
            config.analysis.hdr.clone().unwrap_or_default(),
            config.analysis.dolby_vision.clone(),
            config.tools.hdr10plus_tool.clone(),
        );

        let content_analyzer = ContentAnalyzer::new();

        let crop_detector = if config.analysis.crop_detection.enabled {
            Some(CropDetector::new(config.analysis.crop_detection.clone()))
        } else {
            None
        };

        Ok(Self {
            content_manager,
            content_analyzer,
            crop_detector,
        })
    }

    pub async fn analyze_complete_pipeline(
        &self,
        ffmpeg: &FfmpegWrapper,
        input_path: &Path,
        metadata: &VideoMetadata,
    ) -> Result<CompleteAnalysisResult> {
        // Fast HDR analysis for crop detection threshold selection
        let hdr_analysis = self.content_manager
            .analyze_hdr_only(ffmpeg, input_path)
            .await?;

        // Use HDR result for crop detection (before expensive DV/HDR10+ tools)
        let is_advanced_content = hdr_analysis.metadata.format != crate::hdr::HdrFormat::None;

        // Crop detection
        let (crop_values, crop_sample_timestamps, crop_analysis_result) =
            self.detect_crop_if_enabled(input_path, metadata, is_advanced_content).await?;

        // Complete content analysis, reusing the HDR analysis
        let content_analysis = self.content_manager
            .analyze_content_with_hdr_reuse(ffmpeg, input_path, Some(hdr_analysis))
            .await?;

        Ok(CompleteAnalysisResult {
            content_analysis,
            crop_values,
            crop_sample_timestamps,
            crop_analysis_result,
        })
    }

    async fn detect_crop_if_enabled(
        &self,
        input_path: &Path,
        metadata: &VideoMetadata,
        is_advanced_content: bool,
    ) -> Result<(Option<String>, Vec<f64>, Option<crate::analysis::CropAnalysisResult>)> {
        if let Some(ref crop_detector) = self.crop_detector {
            let crop_analysis = crop_detector
                .detect_crop_values(
                    input_path,
                    metadata.duration,
                    metadata.width,
                    metadata.height,
                    is_advanced_content,
                )
                .await?;

            let sample_timestamps = crop_detector.config
                .get_sample_timestamps(metadata.duration);

            let crop_values = crop_analysis
                .crop_values
                .as_ref()
                .map(|cv| cv.to_ffmpeg_string());

            Ok((crop_values, sample_timestamps, Some(crop_analysis)))
        } else {
            Ok((None, vec![], None))
        }
    }
}
```

**Update**: `src/analysis/mod.rs`
```rust
mod content;
mod crop;
mod dolby_vision;
mod orchestrator;  // Add this line
mod video;

pub use content::{ContentAnalyzer, ContentClassification};
pub use crop::{CropAnalysisResult, CropDetector, CropValues};
pub use dolby_vision::DolbyVisionAnalyzer;
pub use orchestrator::{AnalysisOrchestrator, CompleteAnalysisResult};  // Add this line
pub use video::VideoAnalysis;
```

**Update VideoProcessor**: Replace analysis coordination code (lines 52-84) with:
```rust
impl<'a> VideoProcessor<'a> {
    pub fn new(
        ffmpeg: &'a FfmpegWrapper,
        stream_preservation: &'a StreamPreservation,
        args: &'a CliArgs,
        config: &'a Config,
        profile_manager: &'a mut ProfileManager,
        input_path: &'a Path,
        output_path: &'a Path,
    ) -> Result<Self> {
        let stream_profile_manager = StreamSelectionProfileManager::new(
            config.stream_selection_profiles.clone()
        )?;

        let analysis_orchestrator = AnalysisOrchestrator::new(config)?;

        Ok(Self {
            ffmpeg,
            stream_preservation,
            args,
            config,
            profile_manager,
            stream_profile_manager,
            analysis_orchestrator,  // Add this field
            input_path,
            output_path,
        })
    }

    pub async fn run(&mut self) -> Result<()> {
        let metadata = self.get_metadata().await?;

        // Use the orchestrator instead of manual coordination
        let analysis_result = self.analysis_orchestrator
            .analyze_complete_pipeline(self.ffmpeg, self.input_path, &metadata)
            .await?;

        let metadata_workflow = self.initialize_metadata_workflow().await?;
        let extracted_metadata = metadata_workflow
            .extract_metadata(
                self.input_path,
                &analysis_result.content_analysis.recommended_approach,
                &analysis_result.content_analysis.dolby_vision,
                &analysis_result.content_analysis.hdr_analysis,
            )
            .await?;

        self.log_content_analysis(&metadata, &analysis_result.content_analysis);

        // ... rest of the method continues unchanged
    }
}
```

**Impact**:
- **Lines Reduced**: ~30 lines from VideoProcessor
- **Complexity Reduction**: Analysis logic properly encapsulated
- **Testability**: Analysis can be tested independently
- **Risk Level**: Medium

### Phase 3: Extract Profile Selection

#### 3.1 Create ProfileSelector Service

**Objective**: Centralize profile selection and eliminate remaining duplication

**New File**: `src/config/profile_selector.rs`
```rust
use crate::{
    analysis::{ContentAnalyzer, ContentClassification},
    cli::CliArgs,
    config::{ContentType, EncodingProfile, ProfileManager},
    utils::{ffmpeg::VideoMetadata, Error, Result},
};
use tracing::info;

pub struct ProfileSelector {
    profile_manager: ProfileManager,
    content_analyzer: ContentAnalyzer,
}

impl ProfileSelector {
    pub fn new(mut profile_manager: ProfileManager) -> Self {
        Self {
            profile_manager,
            content_analyzer: ContentAnalyzer::new(),
        }
    }

    pub async fn select_optimal_profile(
        &mut self,
        args: &CliArgs,
        metadata: &VideoMetadata,
    ) -> Result<EncodingProfile> {
        if args.profile == "auto" {
            info!("Auto-selecting profile based on content analysis...");
            self.select_automatic_profile(metadata).await
        } else {
            self.select_named_profile(&args.profile)
        }
    }

    async fn select_automatic_profile(
        &mut self,
        metadata: &VideoMetadata,
    ) -> Result<EncodingProfile> {
        let classification = self.content_analyzer
            .classify_content(metadata)
            .await?;

        if let Some(profile) = self.profile_manager.recommend_profile_for_resolution(
            metadata.width,
            metadata.height,
            classification.content_type,
        ) {
            info!(
                "Selected profile based on content analysis: {} (confidence: {:.1}%)",
                profile.name,
                classification.confidence * 100.0
            );
            Ok(profile.clone())
        } else {
            info!("No specific profile found for content type, using default 'movie' profile");
            self.select_fallback_profile()
        }
    }

    fn select_named_profile(&self, profile_name: &str) -> Result<EncodingProfile> {
        self.profile_manager
            .get_profile(profile_name)
            .cloned()
            .ok_or_else(|| Error::profile(format!("Profile '{}' not found", profile_name)))
    }

    fn select_fallback_profile(&self) -> Result<EncodingProfile> {
        self.profile_manager
            .get_profile("movie")
            .cloned()
            .ok_or_else(|| Error::profile("Default 'movie' profile not found"))
    }

    pub fn list_available_profiles(&self) -> Vec<&String> {
        self.profile_manager.list_profiles()
    }

    pub fn get_profile(&self, name: &str) -> Option<&EncodingProfile> {
        self.profile_manager.get_profile(name)
    }
}
```

**Update**: `src/config/mod.rs`
```rust
mod loader;
mod profiles;
mod profile_selector;  // Add this line
mod stream_profiles;
mod types;

pub use loader::Config;
pub use profiles::{EncodingProfile, ProfileManager};
pub use profile_selector::ProfileSelector;  // Add this line
pub use stream_profiles::{StreamSelectionProfile, StreamSelectionProfileManager};
pub use types::*;
```

**Update VideoProcessor**: Replace profile selection logic with:
```rust
impl<'a> VideoProcessor<'a> {
    pub fn new(
        ffmpeg: &'a FfmpegWrapper,
        stream_preservation: &'a StreamPreservation,
        args: &'a CliArgs,
        config: &'a Config,
        profile_manager: ProfileManager,  // Take ownership instead of mutable reference
        input_path: &'a Path,
        output_path: &'a Path,
    ) -> Result<Self> {
        let stream_profile_manager = StreamSelectionProfileManager::new(
            config.stream_selection_profiles.clone()
        )?;

        let analysis_orchestrator = AnalysisOrchestrator::new(config)?;
        let profile_selector = ProfileSelector::new(profile_manager);

        Ok(Self {
            ffmpeg,
            stream_preservation,
            args,
            config,
            profile_selector,  // Replace profile_manager
            stream_profile_manager,
            analysis_orchestrator,
            input_path,
            output_path,
        })
    }

    pub async fn run(&mut self) -> Result<()> {
        let metadata = self.get_metadata().await?;

        let analysis_result = self.analysis_orchestrator
            .analyze_complete_pipeline(self.ffmpeg, self.input_path, &metadata)
            .await?;

        // Use the selector instead of inline logic
        let selected_profile = self.profile_selector
            .select_optimal_profile(self.args, &metadata)
            .await?;

        // ... rest continues unchanged
    }

    // Remove the select_profile and classify_content_from_metadata methods entirely
}
```

**Impact**:
- **Lines Reduced**: ~40 lines from VideoProcessor
- **Duplication Eliminated**: All content classification logic centralized
- **Improved API**: Cleaner interface for profile selection
- **Risk Level**: Medium

### Phase 4: Extract Encoding Pipeline

#### 4.1 Create EncodingPipeline

**Objective**: Extract encoding orchestration into dedicated component

**New File**: `src/encoding/pipeline.rs`
```rust
use crate::{
    cli::CliArgs,
    config::{Config, EncodingProfile},
    encoding::{
        modes::{Encoder, AbrEncoder, CbrEncoder, CrfEncoder},
        EncodingMode, FilterChain,
    },
    metadata_workflow::{ExtractedMetadata, MetadataWorkflowManager},
    progress::ProgressMonitor,
    stream::preservation::{StreamMapping, StreamPreservation},
    utils::{ffmpeg::VideoMetadata, Error, FfmpegWrapper, FileLogger, Result},
    ContentAnalysisResult,
};
use std::{path::Path, time::Instant};
use tokio::process::Child;

#[derive(Debug)]
pub struct EncodingConfiguration<'a> {
    pub input_path: &'a Path,
    pub output_path: &'a Path,
    pub profile: &'a EncodingProfile,
    pub filter_chain: &'a FilterChain,
    pub stream_mapping: &'a StreamMapping,
    pub metadata: &'a VideoMetadata,
    pub adaptive_crf: f32,
    pub adaptive_bitrate: u32,
    pub encoding_mode: EncodingMode,
    pub custom_title: Option<&'a str>,
    pub external_metadata_params: Option<&'a [(String, String)]>,
    pub passthrough_mode: bool,
}

#[derive(Debug)]
pub struct EncodingResults {
    pub success: bool,
    pub duration: std::time::Duration,
    pub output_size: Option<u64>,
    pub exit_code: Option<i32>,
}

pub struct EncodingPipeline<'a> {
    ffmpeg: &'a FfmpegWrapper,
    stream_preservation: &'a StreamPreservation,
    config: &'a Config,
}

impl<'a> EncodingPipeline<'a> {
    pub fn new(
        ffmpeg: &'a FfmpegWrapper,
        stream_preservation: &'a StreamPreservation,
        config: &'a Config,
    ) -> Self {
        Self {
            ffmpeg,
            stream_preservation,
            config,
        }
    }

    pub async fn execute_encoding(
        &self,
        config: EncodingConfiguration<'_>,
        metadata_workflow: &MetadataWorkflowManager,
        extracted_metadata: &ExtractedMetadata,
    ) -> Result<EncodingResults> {
        let start_time = Instant::now();
        let file_logger = FileLogger::new(config.output_path)?;

        // Log initial settings
        self.log_encoding_settings(&file_logger, &config)?;

        // Determine actual output path (with or without post-processing)
        let needs_post_processing = metadata_workflow.needs_post_processing(extracted_metadata);
        let actual_output_path = if needs_post_processing {
            metadata_workflow.get_temp_output_path(config.output_path)
        } else {
            config.output_path.to_path_buf()
        };

        // Start encoding
        let child = self.start_encoding_process(&actual_output_path, &config).await?;

        // Monitor progress
        let mut progress_monitor = self.create_progress_monitor(&config);
        let status = progress_monitor.monitor_encoding(child).await?;

        // Handle post-processing if needed
        if status.success() && needs_post_processing {
            metadata_workflow
                .inject_metadata(
                    &actual_output_path,
                    &config.output_path.to_path_buf(),
                    extracted_metadata,
                )
                .await?;
        }

        let duration = start_time.elapsed();
        let output_size = std::fs::metadata(config.output_path).map(|m| m.len()).ok();
        let exit_code = status.code();

        // Finalize logging
        self.finalize_logging(&file_logger, status.success(), duration, output_size, exit_code)?;

        Ok(EncodingResults {
            success: status.success(),
            duration,
            output_size,
            exit_code,
        })
    }

    async fn start_encoding_process(
        &self,
        actual_output_path: &Path,
        config: &EncodingConfiguration<'_>,
    ) -> Result<Child> {
        match config.encoding_mode {
            EncodingMode::CRF => {
                CrfEncoder.encode(
                    self.ffmpeg,
                    config.input_path,
                    actual_output_path,
                    config.profile,
                    config.filter_chain,
                    config.stream_mapping,
                    config.metadata,
                    config.adaptive_crf,
                    config.adaptive_bitrate,
                    config.custom_title,
                    Some(&FileLogger::new(config.output_path)?),
                    config.external_metadata_params,
                    config.passthrough_mode,
                ).await
            }
            EncodingMode::ABR => {
                AbrEncoder.encode(
                    self.ffmpeg,
                    config.input_path,
                    actual_output_path,
                    config.profile,
                    config.filter_chain,
                    config.stream_mapping,
                    config.metadata,
                    config.adaptive_crf,
                    config.adaptive_bitrate,
                    config.custom_title,
                    Some(&FileLogger::new(config.output_path)?),
                    config.external_metadata_params,
                    config.passthrough_mode,
                ).await
            }
            EncodingMode::CBR => {
                CbrEncoder::new().encode(
                    self.ffmpeg,
                    config.input_path,
                    actual_output_path,
                    config.profile,
                    config.filter_chain,
                    config.stream_mapping,
                    config.metadata,
                    config.adaptive_crf,
                    config.adaptive_bitrate,
                    config.custom_title,
                    Some(&FileLogger::new(config.output_path)?),
                    config.external_metadata_params,
                    config.passthrough_mode,
                ).await
            }
        }
    }

    fn create_progress_monitor(&self, config: &EncodingConfiguration<'_>) -> ProgressMonitor {
        let source_file_size = std::fs::metadata(config.input_path).map(|m| m.len()).ok();

        let progress_monitor = ProgressMonitor::new(
            config.metadata.duration,
            config.metadata.fps,
            self.ffmpeg.clone(),
            config.encoding_mode,
            source_file_size,
        );

        let total_frames = if config.metadata.fps > 0.0 && config.metadata.duration > 0.0 {
            (config.metadata.duration * config.metadata.fps as f64) as u32
        } else {
            0
        };

        progress_monitor.set_message(&format!(
            "Encoding {} ({}x{}, {:.1}fps, {} frames)",
            config.input_path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy(),
            config.metadata.width,
            config.metadata.height,
            config.metadata.fps,
            total_frames
        ));

        progress_monitor
    }

    fn log_encoding_settings(
        &self,
        file_logger: &FileLogger,
        config: &EncodingConfiguration<'_>,
    ) -> Result<()> {
        file_logger.log_encoding_settings(
            config.input_path,
            config.output_path,
            &config.profile.name,
            config.profile,
            &format!("{:?}", config.encoding_mode),
            config.adaptive_crf,
            config.adaptive_bitrate,
            Some(&config.filter_chain.to_string()),
            &format!("{:?}", config.stream_mapping),
        )
    }

    fn finalize_logging(
        &self,
        file_logger: &FileLogger,
        success: bool,
        duration: std::time::Duration,
        output_size: Option<u64>,
        exit_code: Option<i32>,
    ) -> Result<()> {
        if success {
            if let Some(size) = output_size {
                tracing::info!(
                    "Encoding completed successfully in {:.2}s, output size: {:.2} MB",
                    duration.as_secs_f64(),
                    size as f64 / 1_048_576.0
                );
            } else {
                tracing::info!(
                    "Encoding completed successfully in {:.2}s",
                    duration.as_secs_f64()
                );
            }
            tracing::info!(
                "Encoding log saved to: {}",
                file_logger.get_log_path().display()
            );
        }

        file_logger.log_encoding_complete(success, duration, output_size, exit_code)?;

        if !success {
            return Err(Error::encoding(format!(
                "Encoding failed with exit code: {}",
                exit_code.unwrap_or(-1)
            )));
        }

        Ok(())
    }
}
```

#### 4.2 Simplify VideoProcessor

**Final VideoProcessor** (dramatically reduced):
```rust
use crate::{
    analysis::{AnalysisOrchestrator, CompleteAnalysisResult},
    cli::CliArgs,
    config::{Config, ProfileSelector},
    encoding::{
        pipeline::{EncodingConfiguration, EncodingPipeline},
        FilterBuilder, EncodingMode,
    },
    metadata_workflow::MetadataWorkflowManager,
    stream::preservation::{StreamPreservation, StreamSelectionProfileManager},
    utils::{ffmpeg::VideoMetadata, Error, FfmpegWrapper, Result},
};
use std::path::Path;
use tracing::info;

pub struct VideoProcessor<'a> {
    ffmpeg: &'a FfmpegWrapper,
    stream_preservation: &'a StreamPreservation,
    args: &'a CliArgs,
    config: &'a Config,
    profile_selector: ProfileSelector,
    stream_profile_manager: StreamSelectionProfileManager,
    analysis_orchestrator: AnalysisOrchestrator,
    encoding_pipeline: EncodingPipeline<'a>,
    input_path: &'a Path,
    output_path: &'a Path,
}

impl<'a> VideoProcessor<'a> {
    pub fn new(
        ffmpeg: &'a FfmpegWrapper,
        stream_preservation: &'a StreamPreservation,
        args: &'a CliArgs,
        config: &'a Config,
        profile_manager: ProfileManager,
        input_path: &'a Path,
        output_path: &'a Path,
    ) -> Result<Self> {
        let stream_profile_manager = StreamSelectionProfileManager::new(
            config.stream_selection_profiles.clone()
        )?;

        let analysis_orchestrator = AnalysisOrchestrator::new(config)?;
        let profile_selector = ProfileSelector::new(profile_manager);
        let encoding_pipeline = EncodingPipeline::new(ffmpeg, stream_preservation, config);

        Ok(Self {
            ffmpeg,
            stream_preservation,
            args,
            config,
            profile_selector,
            stream_profile_manager,
            analysis_orchestrator,
            encoding_pipeline,
            input_path,
            output_path,
        })
    }

    pub async fn run(&mut self) -> Result<()> {
        // Get metadata
        let metadata = self.get_metadata().await?;

        // Orchestrate analysis
        let analysis_result = self.analysis_orchestrator
            .analyze_complete_pipeline(self.ffmpeg, self.input_path, &metadata)
            .await?;

        // Initialize metadata workflow
        let metadata_workflow = MetadataWorkflowManager::new(self.config).await?;
        let extracted_metadata = metadata_workflow
            .extract_metadata(
                self.input_path,
                &analysis_result.content_analysis.recommended_approach,
                &analysis_result.content_analysis.dolby_vision,
                &analysis_result.content_analysis.hdr_analysis,
            )
            .await?;

        // Log analysis results
        self.log_content_analysis(&metadata, &analysis_result);

        // Select profile
        let selected_profile = self.profile_selector
            .select_optimal_profile(self.args, &metadata)
            .await?;

        // Calculate adjustments
        let adaptive_crf = selected_profile.base_crf +
            analysis_result.content_analysis.encoding_adjustments.crf_adjustment;
        let adaptive_bitrate = ((selected_profile.base_bitrate as f32)
            * analysis_result.content_analysis.encoding_adjustments.bitrate_multiplier)
            as u32;

        // Log parameter adjustments
        self.log_parameter_adjustments(&analysis_result, &selected_profile, adaptive_crf, adaptive_bitrate);

        // Build filter chain
        let filter_chain = self.build_filter_chain(analysis_result.crop_values.as_deref())?;

        // Get encoding mode
        let encoding_mode = EncodingMode::from_string(&self.args.mode)
            .ok_or_else(|| Error::encoding(format!("Invalid encoding mode: {}", self.args.mode)))?;

        // Analyze streams
        let stream_mapping = self.analyze_streams().await?;

        // Execute encoding
        let encoding_config = EncodingConfiguration {
            input_path: self.input_path,
            output_path: self.output_path,
            profile: &selected_profile,
            filter_chain: &filter_chain,
            stream_mapping: &stream_mapping,
            metadata: &metadata,
            adaptive_crf,
            adaptive_bitrate,
            encoding_mode,
            custom_title: self.args.title.as_deref(),
            external_metadata_params: None, // TODO: Extract from metadata workflow
            passthrough_mode: self.config.analysis.hdr_detection.passthrough_mode,
        };

        let results = self.encoding_pipeline
            .execute_encoding(encoding_config, &metadata_workflow, &extracted_metadata)
            .await?;

        // Cleanup
        metadata_workflow.cleanup().await?;
        extracted_metadata.cleanup();

        if results.success {
            info!("✓ Processing completed successfully");
        }

        Ok(())
    }

    async fn get_metadata(&self) -> Result<VideoMetadata> {
        info!("Getting video metadata for: {}", self.input_path.display());
        self.ffmpeg.get_video_metadata(self.input_path).await
    }

    fn log_content_analysis(&self, metadata: &VideoMetadata, content_analysis: &crate::ContentAnalysisResult) {
        // Simplified logging - details moved to AnalysisOrchestrator
        match &content_analysis.recommended_approach {
            crate::ContentEncodingApproach::SDR => info!("SDR CONTENT DETECTED"),
            crate::ContentEncodingApproach::HDR(hdr_result) => {
                info!("HDR CONTENT DETECTED");
                info!("  Format: {:?}", hdr_result.metadata.format);
                if let Some(ref color_space) = metadata.color_space {
                    info!("  Color Space: {}", color_space);
                }
            }
            crate::ContentEncodingApproach::DolbyVision(dv_info) => {
                info!("DOLBY VISION CONTENT DETECTED");
                info!("  Profile: {}", dv_info.profile.as_str());
            }
            crate::ContentEncodingApproach::DolbyVisionWithHDR10Plus(dv_info, _) => {
                info!("DUAL FORMAT CONTENT DETECTED: DOLBY VISION + HDR10+");
                info!("  Dolby Vision Profile: {}", dv_info.profile.as_str());
            }
        }
    }

    fn log_parameter_adjustments(
        &self,
        analysis_result: &CompleteAnalysisResult,
        selected_profile: &crate::config::EncodingProfile,
        adaptive_crf: f32,
        adaptive_bitrate: u32,
    ) {
        match &analysis_result.content_analysis.recommended_approach {
            crate::ContentEncodingApproach::SDR => {
                info!(
                    "Using standard encoding parameters (SDR): CRF={:.1}, Bitrate={}kbps",
                    adaptive_crf, adaptive_bitrate
                );
            }
            _ => {
                info!("PARAMETER ADJUSTMENTS:");
                info!(
                    "  Base CRF: {} -> Adjusted CRF: {:.1} (+{:.1})",
                    selected_profile.base_crf,
                    adaptive_crf,
                    analysis_result.content_analysis.encoding_adjustments.crf_adjustment
                );
                info!(
                    "  Base Bitrate: {} -> Adjusted Bitrate: {} ({:.1}x multiplier)",
                    selected_profile.base_bitrate,
                    adaptive_bitrate,
                    analysis_result.content_analysis.encoding_adjustments.bitrate_multiplier
                );
            }
        }
    }

    fn build_filter_chain(&self, crop_values: Option<&str>) -> Result<crate::encoding::FilterChain> {
        Ok(FilterBuilder::new(self.config)
            .with_deinterlace(self.args.deinterlace)?
            .with_denoise(self.args.denoise)
            .with_crop(crop_values)?
            .build())
    }

    async fn analyze_streams(&self) -> Result<crate::stream::preservation::StreamMapping> {
        if let Some(profile_name) = &self.args.stream_selection_profile {
            let profile = self.stream_profile_manager.get_profile(profile_name)?;
            self.stream_preservation
                .analyze_streams_with_profile(self.input_path, profile)
                .await
        } else {
            self.stream_preservation
                .analyze_streams(self.input_path)
                .await
        }
    }
}
```

**Impact**:
- **Lines Reduced**: 612 → ~200 lines (67% reduction)
- **Responsibilities Separated**: Each component has a single, clear purpose
- **Testability**: All major components can be tested independently
- **Maintainability**: Changes to specific features don't affect the entire processor
- **Risk Level**: Higher (complete architectural change)

## Risk Assessment

### Phase-by-Phase Risk Analysis

| Phase | Risk Level | Risk Factors | Mitigation Strategies |
|-------|------------|--------------|----------------------|
| **Phase 1** | **Very Low** | - Simple refactoring<br>- No API changes<br>- Well-tested duplicated logic | - Comprehensive unit tests<br>- Manual verification of filtering<br>- Feature flags for new filtering |
| **Phase 2** | **Medium** | - New module introduction<br>- Logic redistribution<br>- Potential integration issues | - Thorough integration testing<br>- Gradual rollout<br>- Monitoring analysis accuracy |
| **Phase 3** | **Medium** | - Profile selection changes<br>- Ownership model changes<br>- API surface modifications | - Extensive profile selection testing<br>- Backward compatibility verification<br>- Performance benchmarking |
| **Phase 4** | **Higher** | - Major architectural change<br>- Complex state management<br>- Multiple component interactions | - Comprehensive system testing<br>- Parallel implementation<br>- Rollback plan preparation |

### Testing Strategy

**Phase 1 Testing**:
```bash
# Test content classification behavior
cargo test content_classification

# Test FFmpeg filtering with real samples
cargo test ffmpeg_message_filtering

# Manual verification
./target/release/ffmpeg-encoder -i test_file.mkv -p auto --debug
```

**Phase 2-4 Testing**:
```bash
# Unit tests for new components
cargo test analysis_orchestrator
cargo test profile_selector
cargo test encoding_pipeline

# Integration tests
cargo test video_processor_integration

# End-to-end tests with various content types
cargo test e2e_sdr_content
cargo test e2e_hdr_content
cargo test e2e_dolby_vision_content
```

### Performance Considerations

**Expected Performance Impact**:
- **Phase 1**: Negligible (may slightly improve due to reduced stderr parsing)
- **Phase 2**: Negligible (same analysis, better organization)
- **Phase 3**: Slight improvement (reduced code paths)
- **Phase 4**: Potential slight improvement (reduced memory allocations)

**Memory Usage**:
- Current: Multiple component instantiations per file
- After Phase 4: Potential reduction through better lifecycle management

## Expected Benefits

### Immediate Benefits (Phase 1)
- **Cleaner User Experience**: FFmpeg diagnostic noise filtered out
- **Code Quality**: Eliminated duplication and simplified maintenance
- **Development Velocity**: Fewer places to maintain identical logic

### Medium-term Benefits (Phases 2-3)
- **Improved Testing**: Individual components can be unit tested
- **Better Error Handling**: Localized error contexts and recovery
- **Easier Feature Addition**: New analysis or profile features have clear homes
- **Reduced Cognitive Load**: Developers can understand components individually

### Long-term Benefits (Phase 4)
- **Maintainable Architecture**: Clear separation of concerns
- **Extensibility**: Easy to add new encoding modes or analysis types
- **Performance Optimization**: Individual components can be optimized independently
- **Team Productivity**: Multiple developers can work on different aspects simultaneously

### Metrics for Success

**Code Metrics**:
- **Lines of Code**: 612 → ~200 (67% reduction in main processor)
- **Cyclomatic Complexity**: Expected 50%+ reduction
- **Test Coverage**: Expected increase from better testability

**Quality Metrics**:
- **Build Time**: Should remain constant or improve slightly
- **Memory Usage**: Expected slight reduction
- **Error Clarity**: Improved error messages and debugging

**Developer Experience**:
- **Feature Development Time**: Expected 30%+ reduction for new features
- **Debugging Time**: Clearer component boundaries should improve troubleshooting
- **Code Review Efficiency**: Smaller, focused changes easier to review

## Implementation Timeline

### Recommended Schedule

**Week 1: Phase 1 Implementation**
- Day 1-2: Remove content classification duplication
- Day 3-4: Implement FFmpeg message filtering
- Day 5: Testing and validation

**Week 2: Phase 2 Implementation**
- Day 1-3: Create and test AnalysisOrchestrator
- Day 4-5: Integration and validation

**Week 3: Phase 3 Implementation**
- Day 1-3: Create and test ProfileSelector
- Day 4-5: Integration and validation

**Week 4: Phase 4 Implementation** *(Optional)*
- Day 1-3: Create EncodingPipeline
- Day 4-5: Simplify VideoProcessor and final integration

**Week 5: Final Testing and Documentation**
- Comprehensive testing across all supported content types
- Documentation updates
- Performance benchmarking

### Rollback Strategy

Each phase should maintain **backward compatibility** until the next phase is proven stable:

1. **Feature Flags**: Use configuration options to toggle between old and new implementations
2. **Parallel Implementation**: Keep old code paths until new ones are validated
3. **Incremental Migration**: Migrate content types progressively (SDR → HDR → DV → HDR10+)
4. **Monitoring**: Add metrics to detect any regressions immediately

### Success Criteria

**Phase 1 Complete When**:
- [ ] All duplicate content classification logic removed
- [ ] FFmpeg diagnostic messages filtered appropriately
- [ ] All existing tests pass
- [ ] Manual testing confirms improved user experience

**Phase 2 Complete When**:
- [ ] AnalysisOrchestrator handles all analysis coordination
- [ ] VideoProcessor analysis logic simplified
- [ ] Analysis accuracy maintained or improved
- [ ] Performance benchmarks show no regression

**Phase 3 Complete When**:
- [ ] ProfileSelector handles all profile selection logic
- [ ] All content classification consolidated
- [ ] Profile selection accuracy maintained
- [ ] API surface area documented and stable

**Phase 4 Complete When**:
- [ ] EncodingPipeline handles all encoding orchestration
- [ ] VideoProcessor reduced to ~200 lines
- [ ] All encoding modes work correctly
- [ ] System-wide performance maintained or improved
- [ ] Comprehensive test coverage achieved

## Conclusion

The FFmpeg Autoencoder's `processing/mod.rs` file exhibits classic symptoms of monolithic growth that impact maintainability, testability, and developer productivity. This refactoring plan provides a systematic approach to address these issues while minimizing risk through incremental implementation.

The **4-phase approach** allows the team to realize benefits early (Phase 1) while maintaining the option to stop at any point where risk/reward balance becomes unfavorable. Even implementing only Phases 1-2 would provide significant benefits with minimal risk.

The proposed architecture separates concerns appropriately while maintaining the tool's core functionality and performance characteristics. This refactoring will position the codebase for future growth and make it significantly easier to maintain and extend.

**Key Recommendation**: Start with Phase 1 as it provides immediate user experience improvements with virtually no risk. Evaluate success and team capacity before proceeding to subsequent phases.