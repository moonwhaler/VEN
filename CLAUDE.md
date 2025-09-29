# FFmpeg Autoencoder - Technical Documentation

This document provides comprehensive technical information for developers and Claude Code users working with the FFmpeg Autoencoder Rust codebase.

## Project Status

Current State: Production-ready video encoding tool focused on reliability and performance
Architecture: Async Rust with comprehensive error handling and type safety
Target: Professional video encoding with intelligent content analysis

### Recent Major Changes
- Unified Content Analysis: Replaced disparate analysis modules with `UnifiedContentManager` for integrated SDR, HDR, Dolby Vision, and HDR10+ detection
- Dolby Vision Integration: Added full support for Dolby Vision metadata preservation and profile-specific encoding adjustments
- HDR10+ Support: Integrated HDR10+ dynamic metadata extraction
- Advanced Parameter Adjustment: Content analysis now drives dynamic adjustments to CRF, bitrate, and VBV settings based on content type
- Processing Module: Introduced centralized `VideoProcessor` for coordinated encoding workflow
- Metadata Workflow: Added dedicated `MetadataWorkflowManager` for HDR10+ and Dolby Vision RPU handling
- Stream Selection Profiles: Added support for custom stream selection profiles with language preferences

## Codebase Architecture

### Directory Structure
```
src/
├── main.rs                      # Application entry point
├── lib.rs                       # Library exports
├── cli/                         # Command-line interface
│   ├── mod.rs                  # CLI module exports
│   ├── args.rs                 # Clap argument definitions
│   └── commands.rs             # Utility commands (list-profiles, show-profile, etc.)
├── config/                      # Configuration management
│   ├── mod.rs                  # Config module exports
│   ├── loader.rs               # YAML config loading and validation
│   ├── types.rs                # Configuration type definitions
│   ├── profiles.rs             # Encoding profile management
│   └── stream_profiles.rs      # Stream selection profile management
├── content_manager.rs           # Unified content analysis orchestrator
├── metadata_workflow.rs         # HDR10+ and Dolby Vision metadata workflow
├── processing/                  # Video processing orchestration
│   └── mod.rs                  # VideoProcessor implementation
├── dolby_vision/                # Dolby Vision detection and analysis
│   ├── mod.rs                  # Module exports
│   ├── rpu.rs                  # RPU data handling
│   └── tools.rs                # dovi_tool integration
├── hdr10plus/                   # HDR10+ metadata extraction
│   ├── mod.rs                  # Module exports
│   ├── manager.rs              # HDR10+ processing manager
│   ├── metadata.rs             # HDR10+ metadata structures
│   └── tools.rs                # hdr10plus_tool integration
├── hdr/                         # HDR analysis and management
│   ├── mod.rs                  # Module exports
│   ├── detection.rs            # HDR detection logic
│   ├── encoding.rs             # HDR encoding parameters
│   ├── metadata.rs             # HDR metadata structures
│   ├── types.rs                # HDR type definitions
│   └── formats/                # Format-specific implementations
│       ├── mod.rs              # Format exports
│       ├── hdr10.rs            # HDR10 format handling
│       ├── hdr10_plus.rs       # HDR10+ format handling
│       └── hlg.rs              # HLG format handling
├── encoding/                    # Core encoding logic
│   ├── mod.rs                  # Encoding module exports
│   ├── modes.rs                # CRF/ABR/CBR implementations
│   ├── filters.rs              # Video filter pipeline (crop, denoise, deinterlace)
│   └── options.rs              # Encoding options validation
├── analysis/                    # Video analysis modules
│   ├── mod.rs                  # Analysis module exports
│   ├── video.rs                # Basic video metadata analysis
│   ├── crop.rs                 # Crop detection analysis
│   ├── content.rs              # Content type classification
│   └── dolby_vision.rs         # Dolby Vision analysis
├── color/                       # Color space handling
│   ├── mod.rs                  # Color module exports
│   ├── spaces.rs               # Color space definitions
│   └── transfers.rs            # Transfer function definitions
├── stream/                      # Stream preservation
│   ├── mod.rs                  # Stream module exports
│   └── preservation.rs         # Audio/subtitle/chapter/metadata handling
├── progress/                    # Enhanced progress tracking
│   └── mod.rs                  # Advanced progress monitoring with stall detection
└── utils/                       # Utilities and helpers
    ├── mod.rs                  # Utils module exports
    ├── ffmpeg.rs               # FFmpeg wrapper and process management
    ├── logging.rs              # Structured logging utilities
    ├── error.rs                # Comprehensive error types
    ├── filesystem.rs           # File system utilities
    └── tool_runner.rs          # External tool execution wrapper
```

### Key Dependencies

```toml
[dependencies]
# CLI and argument parsing
clap = { version = "4.4", features = ["derive", "color", "suggestions"] }

# Async runtime
tokio = { version = "1.35", features = ["full"] }
futures = "0.3"

# Configuration and serialization
serde = { version = "1.0", features = ["derive"] }
serde_yaml = "0.9"
serde_json = "1.0"

# Error handling and logging
anyhow = "1.0"
thiserror = "1.0"
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter", "fmt", "ansi", "chrono"] }
chrono = "0.4"

# File system and path utilities
walkdir = "2.4"
uuid = { version = "1.6", features = ["v4"] }

# Progress bars and terminal UI
indicatif = { version = "0.17", features = ["tokio"] }
console = "0.15"
crossterm = "0.27"

# Regular expressions and string processing
regex = "1.10"
once_cell = "1.19"

# Process management
tokio-process = "0.2"

# Numeric computations
num-traits = "0.2"

# Configuration and environment
dirs = "5.0"
```

## Core Components

### 1. Main Entry Point (`src/main.rs`)

Responsibilities:
- Command-line argument parsing and validation
- Configuration loading and profile management
- File discovery and batch processing
- Encoding orchestration and error handling

Key Functions:
```rust
async fn main() -> Result<()>
async fn handle_encoding(args: &CliArgs, config: &Config) -> Result<()>
```

Process Flow:
1. Parse CLI arguments and load configuration
2. Initialize FFmpeg wrapper and stream preservation
3. Discover input files (single file or directory scan)
4. Load encoding profiles and validate selected profile
5. Process each file using VideoProcessor
6. Track successful/failed encodes and report summary

### 2. Video Processor (`src/processing/mod.rs`)

The VideoProcessor is the central orchestration component that coordinates all encoding steps.

Responsibilities:
- Metadata extraction and analysis
- Content analysis coordination (HDR, DV, HDR10+)
- Crop detection with content-aware thresholds
- Profile selection and parameter adjustment
- Filter chain construction
- Stream mapping analysis
- Encoding execution and monitoring
- Post-processing for metadata injection

Key Methods:
```rust
pub async fn run(&mut self) -> Result<()>
async fn get_metadata(&self) -> Result<VideoMetadata>
async fn detect_crop(&self, is_advanced_content: bool, metadata: &VideoMetadata) -> Result<(Option<String>, Vec<f32>, Option<CropAnalysisResult>)>
async fn select_profile(&self, metadata: &VideoMetadata) -> Result<&EncodingProfile>
async fn analyze_streams(&self) -> Result<StreamMapping>
async fn start_encoding(/* parameters */) -> Result<tokio::process::Child>
```

Process Flow:
1. Extract video metadata using FFprobe
2. Perform fast HDR analysis for crop detection threshold selection
3. Execute crop detection with content-aware thresholds
4. Complete full content analysis (reusing HDR analysis)
5. Initialize metadata workflow manager
6. Extract external metadata (HDR10+, Dolby Vision RPU)
7. Select encoding profile based on content
8. Calculate adaptive CRF and bitrate adjustments
9. Build filter chain and analyze stream mapping
10. Start encoding process (with temp output if post-processing needed)
11. Monitor progress and handle completion
12. Execute post-processing (HDR10+ metadata injection, RPU muxing)

### 3. Configuration System (`src/config/`)

Type-Safe Configuration: All configuration is validated at load time using Serde

Key Structures:
```rust
pub struct Config {
    pub app: AppConfig,                     // Application settings
    pub tools: ToolsConfig,                 // FFmpeg/FFprobe/dovi_tool/hdr10plus_tool paths
    pub logging: LoggingConfig,             // Logging configuration
    pub analysis: AnalysisConfig,           // Video analysis settings
    pub profiles: HashMap<String, RawProfile>,  // Encoding profiles
    pub filters: FiltersConfig,             // Processing filters
    pub stream_selection_profiles: HashMap<String, StreamSelectionProfile>, // Stream profiles
}

pub struct AnalysisConfig {
    pub crop_detection: CropDetectionConfig,
    pub hdr: Option<UnifiedHdrConfig>,
    pub dolby_vision: Option<DolbyVisionConfig>,
}

pub struct ToolsConfig {
    pub ffmpeg: String,
    pub ffprobe: String,
    pub nnedi_weights: Option<String>,
    pub dovi_tool: Option<DoviToolConfig>,
    pub hdr10plus_tool: Option<Hdr10PlusToolConfig>,
}
```

Profile Management:
- 11 pre-defined profiles optimized for different content types
- Automatic profile selection based on content analysis
- x265 parameter tuning with HDR-specific adjustments
- Bitrate scaling for HDR content (typically +30%)
- Profile recommendation based on resolution and content type

Stream Selection Profiles:
- Language-based audio/subtitle selection
- Fallback language chains
- Quality-based codec preferences
- Customizable per-stream-type rules

### 4. Encoding Modes (`src/encoding/modes.rs`)

Three Encoding Strategies:

#### CRF (Constant Rate Factor)
- Quality-based encoding with variable bitrate
- Single-pass with optimized settings
- Best for archival and high-quality encodes

#### ABR (Average Bitrate)
- Two-pass encoding for target bitrate
- Optimized first pass with `no-slow-firstpass`
- Best for size-constrained workflows

#### CBR (Constant Bitrate)
- Two-pass with VBV constraints for streaming
- Buffer management with `vbv-bufsize` and `vbv-maxrate`
- Best for streaming and broadcast applications

Implementation Details:
```rust
pub trait Encoder {
    async fn encode<P: AsRef<Path>>(
        &self,
        ffmpeg: &FfmpegWrapper,
        input_path: P,
        output_path: P,
        profile: &EncodingProfile,
        filters: &FilterChain,
        stream_mapping: &StreamMapping,
        metadata: &VideoMetadata,
        adaptive_crf: f32,
        adaptive_bitrate: u32,
        custom_title: Option<&str>,
        file_logger: Option<&FileLogger>,
        external_metadata_params: Option<&[(String, String)]>,
        hdr_passthrough_mode: bool,
    ) -> Result<tokio::process::Child>;
}

pub struct CrfEncoder;
pub struct AbrEncoder;
pub struct CbrEncoder;
```

### 5. Unified Content Analysis (`src/content_manager.rs`)

The UnifiedContentManager is the core of the intelligent analysis system, orchestrating the detection of SDR, HDR, Dolby Vision, and HDR10+ content.

Key Responsibilities:
- Coordinate Analysis: Sequentially runs HDR, Dolby Vision, and HDR10+ detectors
- Determine Encoding Approach: Selects the best encoding strategy based on a priority system (DV + HDR10+ > DV > HDR10+ > HDR > SDR)
- Calculate Adjustments: Computes dynamic adjustments for CRF, bitrate, and VBV settings based on detected content

```rust
pub struct UnifiedContentManager {
    hdr_manager: HdrManager,
    dv_detector: Option<DolbyVisionDetector>,
    dv_config: Option<DolbyVisionConfig>,
    hdr10plus_manager: Option<Hdr10PlusManager>,
}

pub struct ContentAnalysisResult {
    pub hdr_analysis: HdrAnalysisResult,
    pub dolby_vision: DolbyVisionInfo,
    pub hdr10_plus: Option<Hdr10PlusProcessingResult>,
    pub recommended_approach: ContentEncodingApproach,
    pub encoding_adjustments: EncodingAdjustments,
}

pub enum ContentEncodingApproach {
    SDR,
    HDR(HdrAnalysisResult),
    DolbyVision(DolbyVisionInfo),
    DolbyVisionWithHDR10Plus(DolbyVisionInfo, HdrAnalysisResult),
}

pub struct EncodingAdjustments {
    pub crf_adjustment: f32,
    pub bitrate_multiplier: f32,
    pub encoding_complexity: f32,
    pub requires_vbv: bool,
    pub vbv_bufsize: Option<u32>,
    pub vbv_maxrate: Option<u32>,
    pub recommended_crf_range: (f32, f32),
}
```

Key Methods:
```rust
pub async fn analyze_hdr_only<P: AsRef<Path>>(&self, ffmpeg: &FfmpegWrapper, input_path: P) -> Result<HdrAnalysisResult>
pub async fn analyze_content<P: AsRef<Path>>(&self, ffmpeg: &FfmpegWrapper, input_path: P) -> Result<ContentAnalysisResult>
pub async fn analyze_content_with_hdr_reuse<P: AsRef<Path>>(&self, ffmpeg: &FfmpegWrapper, input_path: P, hdr_result: Option<HdrAnalysisResult>) -> Result<ContentAnalysisResult>
```

#### Dolby Vision Support (`src/dolby_vision/`)
- Profile Detection: Identifies Dolby Vision profiles (5, 7, 8.1, 8.2, 8.4)
- RPU Preservation: Ensures RPU data is correctly handled via dovi_tool
- Profile-Specific Adjustments: Applies different CRF ranges and complexity multipliers for each DV profile
- VBV Constraints: Automatically enforces VBV buffer/maxrate for DV content
- Profile Conversion: Can convert Profile 7 (MEL) to Profile 8.1 (cross-compatible)

#### HDR10+ Support (`src/hdr10plus/`)
- Metadata Extraction: Uses hdr10plus_tool to extract HDR10+ dynamic metadata
- Dual-Format Handling: Manages content that contains both Dolby Vision and HDR10+
- JSON Metadata: Extracts metadata to JSON format for later injection
- Post-Processing: Injects metadata back into encoded file

#### HDR Analysis (`src/hdr/`)
- Format Detection: Identifies HDR10, HDR10+, and HLG formats
- Metadata Parsing: Extracts color space, transfer function, primaries, master display, and MaxCLL/MaxFALL
- Encoding Complexity: Calculates complexity multipliers based on HDR format
- Parameter Adjustments: Provides CRF and bitrate adjustments for HDR content

### 6. Metadata Workflow Manager (`src/metadata_workflow.rs`)

Coordinates the extraction and injection of advanced metadata formats.

Key Responsibilities:
- Determine which metadata needs extraction based on content analysis
- Extract HDR10+ metadata and Dolby Vision RPU before encoding
- Inject metadata back into encoded files after encoding completes
- Manage temporary files for multi-step processing

```rust
pub struct MetadataWorkflowManager {
    hdr10plus_manager: Option<Hdr10PlusManager>,
    dv_tool_runner: Option<DoviToolRunner>,
    temp_dir: PathBuf,
}

pub struct ExtractedMetadata {
    pub hdr10plus: Option<Hdr10PlusProcessingResult>,
    pub dolby_vision_rpu: Option<PathBuf>,
}
```

Key Methods:
```rust
pub async fn extract_metadata<P: AsRef<Path>>(&self, input_path: P, approach: &ContentEncodingApproach, dv_info: &DolbyVisionInfo, hdr_analysis: &HdrAnalysisResult) -> Result<ExtractedMetadata>
pub async fn inject_metadata<P: AsRef<Path>>(&self, temp_output: P, final_output: P, metadata: &ExtractedMetadata) -> Result<()>
pub fn needs_post_processing(&self, metadata: &ExtractedMetadata) -> bool
```

### 7. Stream Preservation (`src/stream/preservation.rs`)

Comprehensive Stream Handling:
- Audio streams: Lossless copy with language preservation
- Subtitle streams: All formats (SRT, ASS, PGS, VobSub, etc.)
- Chapter information: Preserved with timestamps and titles
- Data streams: Fonts, attachments, and other embedded data
- Metadata: Title, creation date, and custom metadata fields

Key Features:
```rust
pub struct StreamMapping {
    pub video_streams: Vec<VideoStream>,
    pub audio_streams: Vec<AudioStream>,
    pub subtitle_streams: Vec<SubtitleStream>,
    pub data_streams: Vec<DataStream>,
    pub mapping_args: Vec<String>,  // FFmpeg arguments for stream mapping
}

pub struct StreamPreservation {
    ffmpeg: FfmpegWrapper,
}

impl StreamPreservation {
    pub async fn analyze_streams<P: AsRef<Path>>(&self, input_path: P, profile: Option<&StreamSelectionProfile>) -> Result<StreamMapping>
}
```

Stream Selection Profiles:
- Preferred languages with fallback chains
- Codec quality preferences (e.g., FLAC > DTS-HD > TrueHD > AC3)
- Stream type specific rules
- Automatic selection when no profile specified

### 8. Progress Monitoring (`src/progress/mod.rs`)

Advanced Progress Tracking:
- File-based progress tracking (`/tmp/ffmpeg_progress_{pid}.txt`)
- Real-time parsing of FFmpeg progress output
- ETA calculations using multiple estimation methods
- Size estimation based on current progress
- Stall detection with configurable thresholds
- FPS and encoding speed statistics

```rust
pub struct ProgressMonitor {
    progress_file: PathBuf,
    total_frames: u64,
    pb: ProgressBar,
    start_time: Instant,
}

impl ProgressMonitor {
    pub async fn monitor_progress(&mut self, child: &mut tokio::process::Child) -> Result<()>
}
```

### 9. FFmpeg Integration (`src/utils/ffmpeg.rs`)

Optimized FFmpeg Wrapper:
```rust
pub struct FfmpegWrapper {
    ffmpeg_path: String,
    ffprobe_path: String,
}

pub struct VideoMetadata {
    pub width: u32,
    pub height: u32,
    pub duration: f64,
    pub frame_rate: f64,
    pub total_frames: u64,
    pub codec: String,
    pub bit_depth: u32,
    pub is_hdr: bool,
    pub color_space: Option<String>,
    pub transfer_function: Option<String>,
    pub color_primaries: Option<String>,
    pub master_display: Option<String>,
    pub max_cll: Option<String>,
}
```

Key Methods:
```rust
pub async fn check_availability(&self) -> Result<()>
pub async fn probe_video<P: AsRef<Path>>(&self, input_path: P) -> Result<VideoMetadata>
pub async fn detect_crop<P: AsRef<Path>>(&self, input_path: P, timestamps: &[f32]) -> Result<Vec<CropDetectionResult>>
pub async fn spawn_ffmpeg(&self, args: Vec<String>) -> Result<tokio::process::Child>
```

Performance Improvements:
- Reduced probe time: 5M vs previous defaults
- Async process management: Non-blocking FFmpeg execution
- Progress file monitoring: Real-time encoding progress
- Error handling: Comprehensive FFmpeg error parsing

## Development Patterns

### Error Handling Strategy

Comprehensive Error Types:
```rust
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("FFmpeg error: {message}")]
    Ffmpeg { message: String },

    #[error("Encoding error: {message}")]
    Encoding { message: String },

    #[error("Configuration error: {message}")]
    Config { message: String },

    #[error("Analysis error: {message}")]
    Analysis { message: String },

    #[error("Validation error: {message}")]
    Validation { message: String },
}
```

Error Recovery:
- Graceful degradation for non-critical failures
- Detailed error context with file paths and settings
- Cleanup routines for temporary files and processes
- Fallback strategies (e.g., yadif when NNEDI weights missing)

### Async Architecture

Key Principles:
- Non-blocking I/O throughout the application
- Process management with Tokio's async process support
- Sequential file processing with async operations
- Resource cleanup with proper async Drop implementations
- Parallel operations where appropriate (e.g., multiple probes)

### Testing Strategy

Current Test Coverage:
- Configuration parsing and validation
- Profile management and parameter building
- Filter chain construction and validation
- Progress parsing and ETA calculations
- Error handling scenarios
- Stream mapping logic
- Content analysis workflows

## Common Development Tasks

### Adding a New Encoding Profile

1. Add profile to `config.yaml`:
```yaml
profiles:
  my_new_profile:
    title: "My New Profile"
    base_crf: 22
    base_bitrate: 10000
    hdr_bitrate: 13000
    content_type: "film"
    x265_params:
      preset: "slow"
      # ... additional parameters
```

2. Update profile recommendation logic in `src/config/profiles.rs` `ProfileManager::recommend_profile_for_resolution()`

3. Add tests for the new profile

### Adding a New Video Filter

1. Update `FiltersConfig` in `src/config/types.rs`
2. Implement filter building in `src/encoding/filters.rs`
3. Add CLI options in `src/cli/args.rs`
4. Update configuration documentation

### Modifying Video Analysis

1. Core analysis logic in `src/analysis/` modules
2. Integration points in `src/content_manager.rs` or `src/processing/mod.rs`
3. Configuration options in `AnalysisConfig`
4. Logging and reporting in analysis results

### Adding Support for New HDR Format

1. Add format variant to `HdrFormat` enum in `src/hdr/types.rs`
2. Implement detection logic in `src/hdr/detection.rs`
3. Add encoding parameter adjustments in `src/hdr/encoding.rs`
4. Update `UnifiedContentManager` to handle new format
5. Add tests for format detection and parameter generation

## Build and Development

### Build Requirements
- Rust 1.70+ with Cargo
- FFmpeg with libx265 support (runtime dependency)
- Standard development tools (git, etc.)
- Optional: dovi_tool for Dolby Vision support
- Optional: hdr10plus_tool for HDR10+ support

### Development Commands
```bash
# Build debug version
cargo build

# Build optimized release
cargo build --release

# Run tests
cargo test

# Check code without building
cargo check

# Lint code
cargo clippy

# Format code
cargo fmt

# Run with sample file
cargo run -- -i sample.mkv -p auto

# Enable debug logging
RUST_LOG=debug cargo run -- -i sample.mkv --debug
```

### Performance Profiling
```bash
# Build with debug symbols for profiling
cargo build --release --config profile.release.debug=true

# Profile with perf (Linux)
perf record target/release/ffmpeg-encoder -i sample.mkv
perf report

# Memory profiling with valgrind
valgrind --tool=massif target/release/ffmpeg-encoder -i sample.mkv
```

## Configuration Reference

### Complete Configuration Schema
```yaml
app:
  temp_dir: "/tmp"
  stats_prefix: "ffmpeg_stats"

tools:
  ffmpeg: "ffmpeg"
  ffprobe: "ffprobe"
  nnedi_weights: null
  dovi_tool:
    path: "/usr/bin/dovi_tool"
    timeout_seconds: 300
  hdr10plus_tool:
    path: "/usr/bin/hdr10plus_tool"
    timeout_seconds: 300

logging:
  level: "info"
  show_timestamps: true
  colored_output: true

analysis:
  crop_detection:
    enabled: true
    sample_count: 8
    sdr_crop_limit: 24
    hdr_crop_limit: 64
    min_pixel_change_percent: 2.0

  hdr:
    enabled: true
    crf_adjustment: 1.0
    bitrate_multiplier: 1.3

  dolby_vision:
    enabled: true
    preserve_profile_7: true
    target_profile: "8.1"
    require_dovi_tool: true
    crf_adjustment: 1.0
    bitrate_multiplier: 1.8
    vbv_crf_bufsize: 160000
    vbv_crf_maxrate: 160000

filters:
  deinterlace:
    primary_method: "nnedi"
    fallback_method: "yadif"
    nnedi_settings:
      field: "auto"

  denoise:
    filter: "hqdn3d"
    params: "1:1:2:2"

stream_selection_profiles:
  default:
    name: "Default Selection"
    preferred_audio_languages: ["eng", "jpn"]
    preferred_subtitle_languages: ["eng"]
    audio_codec_preferences: ["flac", "dts", "truehd", "ac3"]

profiles:
  # Profile definitions with x265 parameters
  # See config.yaml for complete profile examples
```

## Claude Code Integration

This codebase is optimized for Claude Code development with:

### File Organization
- Clear module boundaries with single responsibilities
- Comprehensive error types with context
- Type-safe configuration with validation
- Extensive documentation in code comments

### Development Workflow
- Cargo-based standard Rust development
- Async-first architecture throughout
- Test-driven development patterns
- Configuration-driven behavior

### Common Claude Code Tasks
- Adding new encoding profiles via configuration
- Modifying analysis parameters in analysis modules
- Adjusting progress monitoring in progress modules
- Extending CLI options in cli modules
- Performance optimization in utils modules
- Adding new stream selection rules
- Implementing custom metadata workflows

This technical documentation provides comprehensive understanding of the codebase architecture, development patterns, and integration points for extending the FFmpeg Autoencoder tool.