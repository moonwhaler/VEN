use clap::Parser;
use std::path::PathBuf;
use crate::utils::Result;

#[derive(Parser, Debug)]
#[command(author, version, about)]
#[command(name = "ffmpeg-encoder")]
#[command(about = "Modern Rust-based FFmpeg video encoding automation with intelligent content analysis")]
#[command(long_about = "
A professional-grade Rust implementation of automated video encoding using FFmpeg with x265/HEVC codec.
Provides multi-mode encoding support (CRF/ABR/CBR), intelligent content analysis with automatic profile 
selection, advanced complexity-based parameter optimization, and comprehensive batch processing capabilities.

EXAMPLES:
  # Auto-selection with UUID output
  ffmpeg-encoder -i input.mkv -p auto -m crf

  # Specific profile with custom output  
  ffmpeg-encoder -i input.mkv -o output.mkv -p anime -m crf

  # With complexity analysis and denoising
  ffmpeg-encoder -i input.mkv -p 4k_heavy_grain -m crf --use-complexity --denoise

  # Legacy interlaced footage with neural network deinterlacing
  ffmpeg-encoder -i legacy_footage.mkv -p classic_anime -m crf --deinterlace

  # Batch processing directory
  ffmpeg-encoder -i ~/Videos/Raw/ -p auto -m abr

  # Hardware acceleration with crop detection
  ffmpeg-encoder -i input.mkv -p movie --hardware --use-complexity -m abr
")]
pub struct CliArgs {
    /// Input video file or directory (can be specified multiple times)
    #[arg(short, long, value_name = "PATH", action = clap::ArgAction::Append)]
    pub input: Vec<PathBuf>,

    /// Output file path (optional, auto-generates UUID-based name if not specified)
    #[arg(short, long, value_name = "PATH")]
    pub output: Option<PathBuf>,

    /// Encoding profile to use [anime, classic_anime, 3d_cgi, 3d_complex, movie, movie_mid_grain, 
    /// movie_size_focused, heavy_grain, 4k, 4k_heavy_grain, auto]
    #[arg(short, long, default_value = "auto", value_name = "PROFILE")]
    pub profile: String,

    /// Video title for metadata
    #[arg(short, long, value_name = "TITLE")]
    pub title: Option<String>,

    /// Encoding mode: crf (quality), abr (average bitrate), cbr (constant bitrate)
    #[arg(short, long, default_value = "abr", value_parser = ["crf", "abr", "cbr"])]
    pub mode: String,

    /// Manual crop values in format width:height:x:y
    #[arg(short, long, value_name = "W:H:X:Y")]
    pub crop: Option<String>,

    /// Scale video to specified resolution (widthxheight, -1 for auto)
    #[arg(short, long, value_name = "WxH")]
    pub scale: Option<String>,

    /// Enable complexity analysis for better parameter optimization
    #[arg(long)]
    pub use_complexity: bool,

    /// Enable video denoising (hqdn3d=1:1:2:2)
    #[arg(long)]
    pub denoise: bool,

    /// Enable deinterlacing for interlaced content (NNEDI/yadif)
    #[arg(long)]
    pub deinterlace: bool,

    /// Enable hardware acceleration (CUDA)
    #[arg(long)]
    pub hardware: bool,

    /// Force web search for content classification (overrides config)
    #[arg(long)]
    pub web_search_force: bool,

    /// Disable web search for content classification
    #[arg(long)]
    pub no_web_search: bool,

    /// Configuration file path
    #[arg(long, default_value = "config.yaml", value_name = "FILE")]
    pub config: PathBuf,

    /// Enable verbose logging
    #[arg(short, long)]
    pub verbose: bool,

    /// Enable debug logging
    #[arg(long)]
    pub debug: bool,

    /// Disable colored output
    #[arg(long)]
    pub no_color: bool,

    /// Show help for specific topic [profiles, modes, examples]
    #[arg(long, value_name = "TOPIC")]
    pub help_topic: Option<String>,

    /// List available encoding profiles
    #[arg(long)]
    pub list_profiles: bool,

    /// Show detailed information about a specific profile
    #[arg(long, value_name = "PROFILE")]
    pub show_profile: Option<String>,

    /// Validate configuration file
    #[arg(long)]
    pub validate_config: bool,
}

impl CliArgs {
    pub fn get_log_level(&self) -> &'static str {
        if self.debug {
            "debug"
        } else if self.verbose {
            "info"
        } else {
            "warn"
        }
    }

    pub fn should_use_color(&self) -> bool {
        !self.no_color
    }

    pub fn is_info_command(&self) -> bool {
        self.list_profiles || self.show_profile.is_some() || self.validate_config 
            || self.help_topic.is_some()
    }

    pub fn should_encode(&self) -> bool {
        !self.is_info_command() && !self.input.is_empty()
    }

    pub fn validate(&self) -> Result<()> {
        // Only validate input if we're encoding
        if self.should_encode() {
            if self.input.is_empty() {
                return Err(crate::utils::Error::validation(
                    "At least one input path is required for encoding".to_string()
                ));
            }
            
            // Validate all input paths exist
            for input in &self.input {
                if !input.exists() {
                    return Err(crate::utils::Error::validation(format!(
                        "Input path does not exist: {}",
                        input.display()
                    )));
                }
            }

            if !self.config.exists() {
                return Err(crate::utils::Error::validation(format!(
                    "Configuration file does not exist: {}",
                    self.config.display()
                )));
            }
        }

        // Validate encoding mode
        if !["crf", "abr", "cbr"].contains(&self.mode.as_str()) {
            return Err(crate::utils::Error::validation(format!(
                "Invalid encoding mode: {} (must be crf, abr, or cbr)",
                self.mode
            )));
        }

        // Validate crop format
        if let Some(crop) = &self.crop {
            if !self.is_valid_crop_format(crop) {
                return Err(crate::utils::Error::validation(format!(
                    "Invalid crop format: {} (expected format: width:height:x:y)",
                    crop
                )));
            }
        }

        // Validate scale format
        if let Some(scale) = &self.scale {
            if !self.is_valid_scale_format(scale) {
                return Err(crate::utils::Error::validation(format!(
                    "Invalid scale format: {} (expected format: widthxheight)",
                    scale
                )));
            }
        }

        // Validate profile name
        let valid_profiles = [
            "auto", "anime", "classic_anime", "3d_cgi", "3d_complex", 
            "movie", "movie_mid_grain", "movie_size_focused", "heavy_grain", 
            "4k", "4k_heavy_grain"
        ];
        if !valid_profiles.contains(&self.profile.as_str()) {
            return Err(crate::utils::Error::validation(format!(
                "Invalid profile: {} (valid profiles: {})",
                self.profile,
                valid_profiles.join(", ")
            )));
        }

        Ok(())
    }

    fn is_valid_crop_format(&self, crop: &str) -> bool {
        let parts: Vec<&str> = crop.split(':').collect();
        if parts.len() != 4 {
            return false;
        }
        parts.iter().all(|part| part.parse::<u32>().is_ok())
    }

    fn is_valid_scale_format(&self, scale: &str) -> bool {
        let parts: Vec<&str> = scale.split('x').collect();
        if parts.len() != 2 {
            return false;
        }
        parts.iter().all(|part| {
            *part == "-1" || part.parse::<u32>().is_ok()
        })
    }

    pub fn print_help_topic(&self, topic: &str) {
        match topic.to_lowercase().as_str() {
            "profiles" => {
                println!("AVAILABLE ENCODING PROFILES:\n");
                println!("Content-Specific Profiles:");
                println!("  anime           - Modern anime content (CRF=23, 9000kbps)");
                println!("  classic_anime   - 90s anime with finer details (CRF=22, 10000kbps)");
                println!("  3d_cgi          - 3D CGI Pixar-like (CRF=22, 10000kbps)");
                println!("  3d_complex      - Complex 3D animation Arcane-like (CRF=22, 11000kbps)");
                println!();
                println!("Film Profiles:");
                println!("  movie           - Standard movie (CRF=22, 10000kbps)");
                println!("  movie_mid_grain - Movies with lighter grain (CRF=21, 11000kbps)");
                println!("  movie_size_focused - Standard movie smaller size (CRF=22, 10000kbps)");
                println!("  heavy_grain     - 4K heavy grain preservation (CRF=21, 12000kbps)");
                println!();
                println!("Resolution Profiles:");
                println!("  4k              - General 4K balanced optimization (CRF=22, 15000kbps)");
                println!("  4k_heavy_grain  - 4K heavy grain preservation (CRF=21, 18000kbps)");
                println!();
                println!("Automatic:");
                println!("  auto            - Intelligent profile selection based on content analysis");
            },
            "modes" => {
                println!("ENCODING MODES:\n");
                println!("CRF Mode (-m crf):");
                println!("  Single-pass encoding using only CRF value for quality control");
                println!("  Best for: Archival quality, variable file sizes");
                println!("  Technical: Pure quality-based encoding without bitrate constraints");
                println!();
                println!("ABR Mode (-m abr) [DEFAULT]:");
                println!("  Two-pass average bitrate encoding");
                println!("  Best for: Streaming delivery with predictable file sizes");
                println!("  Pass 1: Fast analysis pass, Pass 2: Quality-optimized encoding");
                println!();
                println!("CBR Mode (-m cbr):");
                println!("  Two-pass constant bitrate with VBV buffer constraints");
                println!("  Best for: Broadcast transmission with constant bandwidth");
                println!("  Technical: Maintains strict bitrate limits for streaming");
            },
            "examples" => {
                println!("USAGE EXAMPLES:\n");
                println!("Basic Usage:");
                println!("  ffmpeg-encoder -i input.mkv -p auto");
                println!("  ffmpeg-encoder -i input.mkv -o output.mkv -p anime -m crf");
                println!();
                println!("Advanced Options:");
                println!("  ffmpeg-encoder -i input.mkv -p 4k_heavy_grain --use-complexity --denoise");
                println!("  ffmpeg-encoder -i input.mkv -p movie --hardware --deinterlace -m abr");
                println!();
                println!("Batch Processing:");
                println!("  ffmpeg-encoder -i ~/Videos/Raw/ -p auto -m crf");
                println!();
                println!("Manual Overrides:");
                println!("  ffmpeg-encoder -i input.mkv -p anime -c 1920:800:0:140 -s 1920x1080");
                println!("  ffmpeg-encoder -i input.mkv -p auto --web-search-force -t \"Movie Title\"");
            },
            _ => {
                println!("Unknown help topic: {}", topic);
                println!("Available topics: profiles, modes, examples");
            }
        }
    }
}