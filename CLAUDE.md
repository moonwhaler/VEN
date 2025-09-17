# FFmpeg Autoencoder - Technical Documentation

This document provides comprehensive technical information for developers and Claude Code users working with the FFmpeg Autoencoder Rust codebase.

## 📋 Project Status

**Current State**: Production-ready video encoding tool focused on reliability and performance  
**Architecture**: Async Rust with comprehensive error handling and type safety  
**Target**: Professional video encoding with intelligent content analysis  

### Recent Major Changes
- ✅ **Unified Content Analysis**: Replaced disparate analysis modules with `UnifiedContentManager` for integrated SDR, HDR, Dolby Vision, and HDR10+ detection.
- ✅ **Dolby Vision Integration**: Added full support for Dolby Vision metadata preservation and profile-specific encoding adjustments.
- ✅ **HDR10+ Support**: Integrated HDR10+ dynamic metadata extraction.
- ✅ **Advanced Parameter Adjustment**: Content analysis now drives dynamic adjustments to CRF, bitrate, and VBV settings based on content type.
- ✅ **Simplified Core Logic**: Removed legacy features like hardware acceleration and external content classification to focus on high-quality software encoding.
- ✅ **FFprobe Optimized**: Maintained optimized FFprobe settings for fast initial analysis.

## 🏗️ Codebase Architecture

### Directory Structure
```
src/
├── main.rs                 # Application entry point
├── cli/                    # Command-line interface
│   ├── args.rs            # Clap argument definitions
│   ├── simple_args.rs     # Simplified CLI parser
│   └── commands.rs        # Utility commands (list-profiles, show-profile, etc.)
├── config/                 # Configuration management
│   ├── loader.rs          # YAML config loading and validation
│   ├── types.rs           # Configuration type definitions
│   └── profiles.rs        # Encoding profile management
├── content_manager.rs     # Unified content analysis orchestrator
├── dolby_vision/           # Dolby Vision detection and analysis
├── hdr10plus/              # HDR10+ metadata extraction
├── hdr/                    # HDR analysis and management
├── encoding/               # Core encoding logic
│   ├── modes.rs           # CRF/ABR/CBR implementations
│   ├── filters.rs         # Video filter pipeline (crop, denoise, deinterlace)
│   └── options.rs         # Encoding options validation
├── analysis/               # Video analysis modules
│   ├── video.rs           # Basic video metadata analysis
│   ├── crop.rs           # Crop detection analysis
│   └── content.rs         # Content type classification
├── stream/                 # Stream preservation
│   └── preservation.rs    # Audio/subtitle/chapter/metadata handling
├── utils/                  # Utilities and helpers
│   ├── ffmpeg.rs          # FFmpeg wrapper and process management
│   ├── progress.rs        # Progress monitoring and ETA calculation
│   ├── logging.rs         # Structured logging utilities
│   └── error.rs           # Comprehensive error types
└── progress/               # Enhanced progress tracking
    └── tracker.rs         # Advanced progress monitoring with stall detection
```

### Key Dependencies

```toml
[dependencies]
# CLI and argument parsing
clap = { version = "4.4", features = ["derive"] }

# Async runtime
tokio = { version = "1.35", features = ["full"] }

# Configuration and serialization
serde = { version = "1.0", features = ["derive"] }
serde_yaml = "0.9"

# Logging and tracing
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["chrono"] }

# Progress and UI
indicatif = "0.17"
console = "0.15"

# Utilities
uuid = { version = "1.6", features = ["v4"] }
regex = "1.10"
once_cell = "1.19"
chrono = { version = "0.4", features = ["serde"] }
```

## 🔧 Core Components

### 1. Main Entry Point (`src/main.rs`)

**Responsibilities**:
- Command-line argument parsing and validation
- Configuration loading and profile management
- File discovery and batch processing
- Encoding orchestration and error handling

**Key Functions**:
```rust
async fn main() -> Result<()>
async fn handle_encoding(args: CliArgs) -> Result<()>
async fn process_single_file(/* params */) -> Result<()>
```

**Process Flow**:
1. Parse CLI arguments and load configuration
2. Discover input files (single file or directory scan)
3. For each file: analyze → select profile → encode → log results
4. Handle errors gracefully with detailed reporting

### 2. Configuration System (`src/config/`)

**Type-Safe Configuration**: All configuration is validated at load time using Serde

**Key Structures**:
```rust
pub struct Config {
    pub app: AppConfig,              // Application settings
    pub tools: ToolsConfig,          // FFmpeg/FFprobe paths
    pub logging: LoggingConfig,      // Logging configuration
    pub progress: ProgressConfig,    // Progress display settings
    pub analysis: AnalysisConfig,    // Video analysis settings
    pub profiles: HashMap<String, RawProfile>,  // Encoding profiles
    pub filters: FiltersConfig,      // Processing filters
}
```

**Profile Management**:
- **11 pre-defined profiles** optimized for different content types
- **Automatic profile selection** based on content analysis
- **x265 parameter tuning** with HDR-specific adjustments
- **Bitrate scaling** for HDR content (typically +30%)

### 3. Encoding Modes (`src/encoding/modes.rs`)

**Three Encoding Strategies**:

#### CRF (Constant Rate Factor)
- **Quality-based encoding** with variable bitrate
- **Single-pass** with optimized settings
- **Best for** archival and high-quality encodes

#### ABR (Average Bitrate) 
- **Two-pass encoding** for target bitrate
- **Optimized first pass** with `no-slow-firstpass`
- **Best for** size-constrained workflows

#### CBR (Constant Bitrate)
- **Two-pass with VBV constraints** for streaming
- **Buffer management** with `vbv-bufsize` and `vbv-maxrate`
- **Best for** streaming and broadcast applications

**Implementation Details**:
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
    ) -> Result<tokio::process::Child>;
}
```

### 4. Unified Content Analysis (`src/content_manager.rs`)

The `UnifiedContentManager` is the core of the intelligent analysis system, orchestrating the detection of SDR, HDR, Dolby Vision, and HDR10+ content.

**Key Responsibilities**:
- **Coordinate Analysis**: Sequentially runs HDR, Dolby Vision, and HDR10+ detectors.
- **Determine Encoding Approach**: Selects the best encoding strategy based on a priority system (e.g., DV + HDR10+ > DV > HDR10+ > HDR > SDR).
- **Calculate Adjustments**: Computes dynamic adjustments for CRF, bitrate, and VBV settings based on the detected content.

```rust
pub struct UnifiedContentManager {
    hdr_manager: HdrManager,
    dv_detector: Option<DolbyVisionDetector>,
    hdr10plus_manager: Option<Hdr10PlusManager>,
}

pub struct ContentAnalysisResult {
    pub hdr_analysis: HdrAnalysisResult,
    pub dolby_vision: DolbyVisionInfo,
    pub hdr10_plus: Option<Hdr10PlusProcessingResult>,
    pub recommended_approach: ContentEncodingApproach,
    pub encoding_adjustments: EncodingAdjustments,
}
```

#### Dolby Vision Support (`src/dolby_vision/`)
- **Profile Detection**: Identifies Dolby Vision profiles (5, 7, 8.1, 8.2, 8.4).
- **RPU Preservation**: Ensures RPU data is correctly handled.
- **Profile-Specific Adjustments**: Applies different CRF ranges and complexity multipliers for each DV profile, ensuring compliance and quality.
- **VBV Constraints**: Automatically enforces VBV buffer/maxrate for DV content.

#### HDR10+ Support (`src/hdr10plus/`)
- **Metadata Extraction**: Uses external tools to extract HDR10+ dynamic metadata.
- **Dual-Format Handling**: Manages content that contains both Dolby Vision and HDR10+.

#### Legacy Analysis (`src/analysis/`)
- **Automatic Crop Detection**: The intelligent crop detector remains a key part of the analysis pipeline, but manual override has been removed.
- **Content Classification**: The simple bitrate-per-pixel heuristic is still used for automatic profile selection when `-p auto` is specified.

### 5. Stream Preservation (`src/stream/preservation.rs`)

**Comprehensive Stream Handling**:
- **Audio streams**: Lossless copy with language preservation
- **Subtitle streams**: All formats (SRT, ASS, PGS, VobSub, etc.)
- **Chapter information**: Preserved with timestamps and titles
- **Data streams**: Fonts, attachments, and other embedded data
- **Metadata**: Title, creation date, and custom metadata fields

**Key Features**:
```rust
pub struct StreamMapping {
    pub video_streams: Vec<VideoStream>,
    pub audio_streams: Vec<AudioStream>,
    pub subtitle_streams: Vec<SubtitleStream>,
    pub data_streams: Vec<DataStream>,
    pub mapping_args: Vec<String>,  // FFmpeg arguments for stream mapping
}
```

### 6. Progress Monitoring (`src/utils/progress.rs`, `src/progress/tracker.rs`)

**Dual Progress Systems**:

#### Basic Progress Monitor (`progress.rs`)
- **File-based progress tracking** (`/tmp/ffmpeg_progress_{pid}.txt`)
- **Real-time parsing** of FFmpeg progress output
- **ETA calculations** using multiple estimation methods
- **Size estimation** based on current progress

#### Enhanced Progress Tracker (`progress/tracker.rs`)
- **Stall detection** with configurable thresholds
- **Multiple ETA algorithms** (time-based, frame-based, speed-adjusted)
- **Advanced statistics** (FPS, encoding speed, bitrate)
- **Regex-based parsing** for reliable progress extraction

### 7. FFmpeg Integration (`src/utils/ffmpeg.rs`)

**Optimized FFmpeg Wrapper**:
```rust
pub struct FfmpegWrapper {
    ffmpeg_path: String,
    ffprobe_path: String,
}

// Optimized probe settings for fast analysis
let output = TokioCommand::new(&self.ffprobe_path)
    .args([
        "-v", "error",
        "-analyzeduration", "5M",    // Reduced from 100M
        "-probesize", "5M",          // Reduced from 50M
        "-print_format", "json", 
        "-show_format",
        "-show_streams",
    ])
```

**Performance Improvements**:
- **Reduced probe time**: 5M vs previous 100M/50M settings
- **Async process management**: Non-blocking FFmpeg execution
- **Progress file monitoring**: Real-time encoding progress
- **Error handling**: Comprehensive FFmpeg error parsing

## 🎯 Development Patterns

### Error Handling Strategy

**Comprehensive Error Types**:
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
    
    // ... additional error types
}
```

**Error Recovery**:
- **Graceful degradation** for non-critical failures
- **Detailed error context** with file paths and settings
- **Cleanup routines** for temporary files and processes

### Async Architecture

**Key Principles**:
- **Non-blocking I/O** throughout the application
- **Process management** with Tokio's async process support
- **Sequential file processing** with async operations
- **Resource cleanup** with proper async Drop implementations

### Testing Strategy

**Current Test Coverage**:
- **Configuration parsing** and validation
- **Profile management** and parameter building
- **Filter chain construction** and validation
- **Progress parsing** and ETA calculations
- **Error handling** scenarios

**Test Examples**:
```rust
#[test]
fn test_encoding_profile_from_raw() {
    let raw = create_test_raw_profile();
    let profile = EncodingProfile::from_raw("test".to_string(), raw).unwrap();
    
    assert_eq!(profile.name, "test");
    assert_eq!(profile.content_type, ContentType::Film);
}

#[test]  
fn test_hdr_parameter_injection() {
    // Test HDR-specific x265 parameter injection
}
```

## 🔄 Common Development Tasks

### Adding a New Encoding Profile

1. **Add profile to `config.yaml`**:
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

2. **Update profile recommendation logic** in `ProfileManager::recommend_profile_for_resolution()`

3. **Add tests** for the new profile in `src/config/profiles.rs`

### Adding a New Video Filter

1. **Update `FiltersConfig`** in `src/config/types.rs`
2. **Implement filter building** in `src/encoding/filters.rs`
3. **Add CLI options** in `src/cli/args.rs`
4. **Update configuration documentation**

### Modifying Video Analysis

1. **Core analysis logic** in `src/analysis/` modules
2. **Integration points** in `src/main.rs` `process_single_file()`
3. **Configuration options** in `AnalysisConfig`
4. **Logging and reporting** in analysis results

## 🚫 Evolved and Deprecated Features

### Feature Evolution
Many features previously listed as "removed" have evolved into more sophisticated systems:

- **Content Adaptation**: Replaced by the `UnifiedContentManager`, which provides much more advanced, content-aware parameter adjustments for HDR, DV, and HDR10+.
- **Complexity Analysis**: The core concept is now integrated into the `UnifiedContentManager`'s encoding adjustments, which consider content complexity implicitly.
- **Advanced Content Classification**: Superseded by the direct detection of advanced formats like Dolby Vision and HDR10+.

### Deprecated Features
These features have been fully removed to streamline the tool:

- **Hardware Acceleration**: The tool is now focused exclusively on high-quality software encoding with x265.
- **Web Search Integration**: All content analysis is performed locally.
- **Video Scaling**: The tool is designed to preserve the original resolution.

## 🛠️ Build and Development

### Build Requirements
- **Rust 1.70+** with Cargo
- **FFmpeg** with libx265 support (runtime dependency)
- **Standard development tools** (git, etc.)

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

## 📊 Configuration Reference

### Complete Configuration Schema
```yaml
app:
  temp_dir: "/tmp"                    # Temporary file directory
  stats_prefix: "ffmpeg2pass"         # Two-pass statistics file prefix

tools:
  ffmpeg: "/usr/bin/ffmpeg"           # FFmpeg binary path
  ffprobe: "/usr/bin/ffprobe"         # FFprobe binary path
  nnedi_weights: null                 # Optional NNEDI3 weights file

logging:
  level: "info"                       # Log level (trace/debug/info/warn/error)
  show_timestamps: true               # Include timestamps in logs
  colored_output: true                # Enable colored console output

progress:
  update_interval_ms: 1000            # Progress update frequency (ms)
  stall_detection_seconds: 30         # Stall detection threshold
  show_eta: true                      # Display ETA in progress
  show_file_size: true                # Display file size estimates

analysis:
  crop_detection:
    enabled: true                     # Enable automatic crop detection
    sample_count: 3                   # Number of temporal samples
    sdr_crop_limit: 24                # SDR crop threshold (pixels)
    hdr_crop_limit: 64                # HDR crop threshold (pixels)  
    min_pixel_change_percent: 1.0     # Minimum change threshold
  
  hdr_detection:
    enabled: true                     # Enable HDR content detection
    color_space_patterns:             # Color space patterns for HDR
      - "bt2020"
      - "rec2020"
    transfer_patterns:                # Transfer function patterns for HDR
      - "smpte2084"
      - "arib-std-b67"
    crf_adjustment: 2.0               # CRF adjustment for HDR content

  dolby_vision:
    enabled: true
    crf_adjustment: 1.0
    bitrate_multiplier: 1.8
    vbv_bufsize: 160000
    vbv_maxrate: 160000
    profile_specific_adjustments: true

  hdr10_plus:
    enabled: true
    temp_dir: "/tmp/hdr10plus"

filters:
  deinterlace:
    primary_method: "nnedi"           # Primary deinterlacing method
    fallback_method: "yadif"          # Fallback deinterlacing method
    nnedi_settings:
      field: "auto"                   # NNEDI field processing mode
  
  denoise:
    filter: "hqdn3d"                  # Denoising filter name
    params: "1:1:2:2"                 # Filter parameters

profiles:
  # Profile definitions with x265 parameters
  # See config.yaml for complete profile examples
```

## 🎭 Claude Code Integration

This codebase is optimized for Claude Code development with:

### File Organization
- **Clear module boundaries** with single responsibilities
- **Comprehensive error types** with context
- **Type-safe configuration** with validation
- **Extensive documentation** in code comments

### Development Workflow  
- **Cargo-based** standard Rust development
- **Async-first** architecture throughout
- **Test-driven** development patterns
- **Configuration-driven** behavior

### Common Claude Code Tasks
- **Adding new encoding profiles** via configuration
- **Modifying analysis parameters** in analysis modules  
- **Adjusting progress monitoring** in progress modules
- **Extending CLI options** in cli modules
- **Performance optimization** in utils modules

This technical documentation should provide Claude Code users with comprehensive understanding of the codebase architecture, development patterns, and integration points for extending the FFmpeg Autoencoder tool.
