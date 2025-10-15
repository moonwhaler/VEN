# VEN - Professional FFmpeg Video Encoder

Rust-based video encoding tool built on FFmpeg and x265/HEVC for batch processing with intelligent content analysis.

## Quick Start

```bash
# Auto-detect content and encode
./ffmpeg-encoder -i input.mkv

# Specify profile and mode
./ffmpeg-encoder -i input.mkv -p anime -m crf

# Batch process directory
./ffmpeg-encoder -i /path/to/videos/ -p auto
```

## Key Features

- **Three encoding modes**: CRF (quality), ABR (average bitrate), CBR (constant bitrate)
- **HDR support**: Dolby Vision (profiles 5, 7, 8.1, 8.2, 8.4) and HDR10+ with metadata preservation
- **Auto crop detection**: Multi-sample temporal analysis with HDR/SDR-specific thresholds
- **Stream preservation**: Losslessly copies all audio, subtitles, chapters, and metadata
- **11 content-specific profiles**: From anime to heavy grain 4K content
- **Preview mode**: Test encoding settings on frames or segments before full encode
- **Processing filters**: NNEDI3/yadif deinterlacing, hqdn3d denoising

## Installation

**Requirements:**
- Rust 1.70+ with Cargo
- FFmpeg with libx265 support
- Optional: `dovi_tool` (Dolby Vision), `hdr10plus_tool` (HDR10+), `mkvmerge` (from mkvtoolnix)

### Quick Build

**Linux/macOS:**
```bash
git clone https://github.com/user/ffmpeg_autoencoder_rust.git
cd ffmpeg_autoencoder_rust

# Production build (optimized, recommended)
./build.sh production

# Or development build (faster compile, includes debug symbols)
./build.sh dev
```

**Windows:**
```cmd
git clone https://github.com/user/ffmpeg_autoencoder_rust.git
cd ffmpeg_autoencoder_rust

REM Production build (optimized, recommended)
build.bat production

REM Or development build (faster compile, includes debug symbols)
build.bat dev
```

**Manual build (all platforms):**
```bash
# Production build
cargo build --release
# Binary: target/release/ffmpeg-encoder

# Development build
cargo build
# Binary: target/debug/ffmpeg-encoder
```

The build scripts will check for required dependencies and provide helpful error messages if anything is missing.

## Usage

### Basic Commands
```bash
# Auto mode (detects best profile)
./ffmpeg-encoder -i input.mkv

# Specific profile
./ffmpeg-encoder -i input.mkv -p anime -m crf

# Batch processing
./ffmpeg-encoder -i /videos/ -p auto

# Custom output path
./ffmpeg-encoder -i input.mkv -o /output/path.mkv
```

### Preview Mode
Test encoding settings before processing the full file:

```bash
# Generate single frame preview at 60s
./ffmpeg-encoder -i input.mkv -p anime --preview --preview-time 60

# Generate 10-second segment preview (30s-40s)
./ffmpeg-encoder -i input.mkv -p anime --preview --preview-range 30-40

# Compare multiple profiles
./ffmpeg-encoder -i input.mkv --preview --preview-time 60 --preview-profile anime_comparison
```

Preview outputs are saved as `{UUID}_preview_{profile}_{timestamp}.{ext}` in the same directory.

### Processing Filters
```bash
# Legacy interlaced content
./ffmpeg-encoder -i old_dvd.avi --deinterlace -p classic_anime

# Heavy grain with denoising
./ffmpeg-encoder -i grainy_film.mkv --denoise -p heavy_grain

# CBR for streaming
./ffmpeg-encoder -i input.mkv -p movie -m cbr
```

## Profiles

View available profiles:
```bash
./ffmpeg-encoder --list-profiles
./ffmpeg-encoder --show-profile anime
```

**Content-optimized profiles:**
- `movie`, `movie_mid_grain`, `movie_size_focused` - Live-action films
- `heavy_grain` - High-grain preservation
- `anime`, `classic_anime` - 2D animation
- `3d_cgi`, `3d_complex` - 3D animated content
- `4k`, `4k_heavy_grain` - 4K content
- `auto` - Automatic profile selection

## Stream Selection

Control which audio/subtitle streams to include:

```bash
# English only
./ffmpeg-encoder -i input.mkv -s english_only

# Multiple languages
./ffmpeg-encoder -i input.mkv -s multilang

# View available profiles
./ffmpeg-encoder --list-stream-profiles
```

## Configuration

The app automatically searches for `config.yaml` in this order:
1. Path specified via `--config` (if provided)
2. `config/` subdirectory next to the binary
3. User config directory: `~/.config/ffmpeg-encoder/` (Linux), `~/Library/Application Support/ffmpeg-encoder/` (macOS), `%APPDATA%\ffmpeg-encoder\` (Windows)
4. Falls back to embedded default configuration

**Key settings:**
- **Tool paths**: FFmpeg, FFprobe, dovi_tool, hdr10plus_tool, mkvmerge
- **Analysis**: Crop detection thresholds, HDR/Dolby Vision handling
- **Filters**: Deinterlacing (NNEDI3/yadif), denoising (hqdn3d)
- **Profiles**: Custom encoding profiles with x265 parameters
- **Preview profiles**: Define comparison groups for preview mode
- **Stream selection**: Audio/subtitle filtering rules

Validate your config:
```bash
./ffmpeg-encoder --validate-config
```

## HDR & Dolby Vision

The tool automatically detects and handles HDR content:

- **HDR10**: Preserves static HDR metadata
- **HDR10+**: Extracts and re-injects dynamic metadata using `hdr10plus_tool`
- **Dolby Vision**: Converts profiles for compatibility (e.g., Profile 7 → 8.1), preserves RPU data using `dovi_tool`

All HDR processing is automatic - just encode as normal. The tool applies appropriate bitrate and CRF adjustments per profile.

## Output

**File naming:**
- Auto-generated: `{original_filename}_{UUID}.{ext}` (prevents conflicts, preserves original name)
- Custom: Use `-o` flag
- Logs: `{original_filename}_{UUID}.log` with detailed encoding information

**Progress display:**
Real-time progress bar with FPS, speed, ETA, and file size estimates.

## Help

```bash
# List available profiles
./ffmpeg-encoder --list-profiles

# Show profile details
./ffmpeg-encoder --show-profile anime

# Validate configuration
./ffmpeg-encoder --validate-config
```

## Advanced Usage

**Custom configuration:**
```bash
./ffmpeg-encoder --config /path/to/custom.yaml -i input.mkv
```

**Logging:**
```bash
# Verbose output
./ffmpeg-encoder -i input.mkv -v

# Debug output
./ffmpeg-encoder -i input.mkv --debug
```

## Technical Details

**Architecture:**
- Async I/O with Tokio
- Type-safe configuration (Serde YAML)
- Multi-pass encoding for ABR/CBR
- Unified content analysis pipeline
- Comprehensive error handling with context

**Processing pipeline:**
1. Content analysis (resolution, HDR detection, crop detection)
2. Profile selection (auto or manual)
3. Stream mapping (audio/subtitle filtering)
4. Encoding (CRF/ABR/CBR with x265)
5. HDR metadata processing (Dolby Vision/HDR10+)
6. Output verification and logging

## License

MIT License - see [LICENSE](LICENSE) file for details.