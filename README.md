# VEN - The ffmpeg video encoder

A professional Rust-based video encoding tool built around FFmpeg and x265, designed for reliable batch processing with intelligent content analysis and comprehensive stream preservation.

## 🚀 Features

### Core Encoding
- **Three encoding modes**: CRF (quality-based), ABR (average bitrate), CBR (constant bitrate)
- **x265/HEVC encoding** with professionally tuned parameters
- **Multi-pass encoding** for ABR/CBR modes with optimized first-pass settings
- **Stream preservation** - losslessly preserves all audio, subtitle, data streams, chapters, and metadata

### Intelligent Video Analysis
- **Unified Content Analysis**: Automatically detects SDR, HDR10, HDR10+, and Dolby Vision to apply optimal encoding parameters.
- **Dolby Vision Support**: Preserves Dolby Vision metadata (Profiles 5, 7, 8.1, 8.2, 8.4) with profile-specific CRF and VBV adjustments.
- **HDR10+ Support**: Extracts and preserves HDR10+ dynamic metadata.
- **Dual Format Handling**: Special conservative settings for content with both Dolby Vision and HDR10+.
- **Automatic crop detection** with multi-temporal sampling and HDR/SDR-specific thresholds.
- **Content classification** for optimal profile selection.
- **Comprehensive metadata extraction** with optimized FFprobe settings.

### Professional Profiles
11 content-specific encoding profiles:
- **movie** - Standard live-action films (CRF=22, 10Mbps)
- **movie_mid_grain** - Films with moderate grain (CRF=21, 11Mbps)  
- **movie_size_focused** - Size-optimized films
- **heavy_grain** - Heavy grain preservation (CRF=21, 12Mbps)
- **3d_cgi** - Pixar-like 3D animation
- **3d_complex** - Complex 3D content like Arcane (CRF=22, 11Mbps)
- **anime** - Modern 2D animation (CRF=23, 9Mbps)
- **classic_anime** - Traditional animation with grain preservation
- **4k** - General 4K content (15Mbps)
- **4k_heavy_grain** - 4K heavy grain preservation (18Mbps)
- **auto** - Automatic profile selection based on content analysis

### Video Processing Pipeline  
- **Deinterlacing** - NNEDI3 primary with yadif fallback
- **Denoising** - Configurable hqdn3d temporal/spatial filtering
- **Crop detection** - Intelligent black bar removal with validation
- **Progress monitoring** - Real-time encoding progress with ETA calculations

## 📦 Installation

### Prerequisites
- **Rust** 1.70+ (with Cargo)
- **FFmpeg** with libx265 support
- **FFprobe** (included with FFmpeg)

### Build from Source
```bash
git clone https://github.com/user/ffmpeg_autoencoder_rust.git
cd ffmpeg_autoencoder_rust
cargo build --release
```

The binary will be created at `target/release/ffmpeg-encoder`.

## 🛠️ Usage

### Basic Encoding
```bash
# Auto-detect profile and encode with default settings
./ffmpeg-encoder -i input.mkv

# Specify profile and encoding mode
./ffmpeg-encoder -i input.mkv -p anime -m crf

# Batch processing directory
./ffmpeg-encoder -i /path/to/videos/ -p auto -m abr
```

### Advanced Options
```bash
# CBR encoding for streaming
./ffmpeg-encoder -i input.mkv -p movie -m cbr

# Legacy interlaced content
./ffmpeg-encoder -i old_content.avi --deinterlace --denoise -p classic_anime

# Custom output location
./ffmpeg-encoder -i input.mkv -o /custom/path/output.mkv
```

## 🎛️ Command Line Options

### Basic Options
- `-i, --input <PATH>` - Input file or directory (required)
- `-o, --output <PATH>` - Output file path (optional, generates UUID names)
- `-p, --profile <PROFILE>` - Encoding profile (default: auto)
- `-m, --mode <MODE>` - Encoding mode: crf/abr/cbr (default: abr)

### Processing Options  
- `--denoise` - Enable video denoising (hqdn3d filter)
- `--deinterlace` - Enable NNEDI3/yadif deinterlacing

### Utility Commands
- `--list-profiles` - Show all available encoding profiles
- `--show-profile <NAME>` - Display detailed profile information
- `--validate-config` - Validate configuration file
- `--help-topic <TOPIC>` - Get help on specific topics (profiles, modes, examples)

### Configuration
- `--config <FILE>` - Custom configuration file path (default: config.yaml)
- `-v, --verbose` - Enable verbose logging
- `--debug` - Enable debug logging
- `--no-color` - Disable colored output

## ⚙️ Configuration

The tool uses a YAML configuration file (`config.yaml`) with the following structure:

### Application Settings
```yaml
app:
  temp_dir: "/tmp"
  stats_prefix: "ffmpeg2pass"
```

### Tool Paths
```yaml
tools:
  ffmpeg: "/usr/bin/ffmpeg" 
  ffprobe: "/usr/bin/ffprobe"
  nnedi_weights: null  # Optional NNEDI weights file
```

### Video Analysis
```yaml
analysis:
  crop_detection:
    enabled: true
    sample_count: 3              # Number of temporal samples
    sdr_crop_limit: 24           # SDR content threshold
    hdr_crop_limit: 64           # HDR content threshold
    min_pixel_change_percent: 1.0
  
  hdr_detection:
    enabled: true
    color_space_patterns: ["bt2020", "rec2020"]
    transfer_patterns: ["smpte2084", "arib-std-b67"] 
    crf_adjustment: 2.0          # CRF increase for HDR content

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
```

### Processing Filters
```yaml
filters:
  deinterlace:
    primary_method: "nnedi"
    fallback_method: "yadif"
    nnedi_settings:
      field: "auto"
  
  denoise:
    filter: "hqdn3d"
    params: "1:1:2:2"           # luma_spatial:chroma_spatial:luma_temporal:chroma_temporal
```

### Encoding Profiles
Each profile defines content-specific parameters:
```yaml
profiles:
  movie:
    title: "Standard Movie"
    base_crf: 22
    base_bitrate: 10000          # kbps for ABR/CBR modes
    hdr_bitrate: 13000           # kbps for HDR content
    content_type: "film"
    x265_params:
      preset: "slow"
      tune: "grain"
      # ... extensive x265 parameters
```

## 🎨 Examples

### Content-Specific Encoding
```bash
# Anime content with size optimization
./ffmpeg-encoder -i "Your Name (2016).mkv" -p anime -m crf

# Heavy grain film preservation
./ffmpeg-encoder -i "Saving Private Ryan.mkv" -p heavy_grain

# 4K content with grain preservation
./ffmpeg-encoder -i "4K_Movie.mkv" -p 4k_heavy_grain

# 3D animated content
./ffmpeg-encoder -i "Pixar_Movie.mkv" -p 3d_cgi
```

### Batch Processing
```bash
# Process entire directory with automatic profile detection
./ffmpeg-encoder -i ~/Movies/ToEncode/ -p auto

# Size-focused batch processing
./ffmpeg-encoder -i ~/Movies/ -p movie_size_focused -m crf
```

### Advanced Processing
```bash
# Legacy content with full restoration
./ffmpeg-encoder -i old_tv_show.avi --deinterlace --denoise -p classic_anime

# Streaming-optimized CBR encoding
./ffmpeg-encoder -i content.mkv -p movie -m cbr
```

## 🔍 Output and Logging

### File Naming
- **Auto-generated names**: `{UUID}.mkv` to prevent conflicts
- **Custom output**: Use `-o` flag for specific output path
- **Batch processing**: Creates files alongside originals with UUID names

### Logging
The tool creates detailed per-file logs (`{output}.log`) containing:
- **Input/output paths** and selected profile information
- **Video analysis results** (resolution, duration, HDR detection, crop detection)
- **Encoding settings** (adaptive CRF/bitrate, filters, x265 parameters)
- **Stream mapping** (audio/subtitle/data stream preservation)
- **Performance metrics** (encoding duration, output size, completion status)

### Progress Monitoring
Real-time progress display showing:
- **Progress bar** with percentage completion
- **Current FPS** and encoding speed multiplier
- **ETA calculations** using multiple estimation methods
- **File size estimates** based on current progress

## 🏗️ Architecture

### Core Modules
- **CLI Interface** (`src/cli/`) - Clap-based command parsing with validation
- **Configuration** (`src/config/`) - Type-safe YAML configuration management
- **Encoding Modes** (`src/encoding/`) - CRF/ABR/CBR implementations with multi-pass support
- **Content Management** (`src/content_manager.rs`) - Unified analysis for SDR, HDR, Dolby Vision, and HDR10+.
- **Video Analysis** (`src/analysis/`) - Crop detection, HDR detection, content classification
- **Dolby Vision** (`src/dolby_vision/`) - Dolby Vision detection and metadata handling.
- **HDR10+** (`src/hdr10plus/`) - HDR10+ metadata extraction and processing.
- **Stream Processing** (`src/stream/`) - Lossless stream preservation and metadata handling
- **Utilities** (`src/utils/`) - FFmpeg wrapper, progress monitoring, error handling

### Key Features
- **Async Architecture** - Non-blocking I/O throughout using Tokio
- **Type Safety** - Comprehensive Rust type system with Serde for configuration
- **Error Handling** - Detailed error types with context and recovery
- **Performance** - Optimized FFprobe settings and efficient progress parsing
- **Stream Preservation** - Complete audio/subtitle/chapter/metadata retention

## 🤝 Contributing

1. Fork the repository
2. Create a feature branch (`git checkout -b feature/amazing-feature`)
3. Commit your changes (`git commit -m 'Add amazing feature'`)
4. Push to the branch (`git push origin feature/amazing-feature`)
5. Open a Pull Request

## 📄 License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## ✨ Acknowledgments

- **FFmpeg team** for the incredible media processing framework
- **x265 developers** for the high-quality HEVC encoder
- **Rust community** for excellent async and CLI libraries
