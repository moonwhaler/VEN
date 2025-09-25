use crate::utils::Result;
use clap::Parser;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(author, version, about)]
#[command(name = "ffmpeg-encoder")]
#[command(
    about = "Modern Rust-based FFmpeg video encoding automation with intelligent content analysis"
)]
#[command(long_about = "
A professional-grade Rust implementation of automated video encoding using FFmpeg with x265/HEVC codec.
Provides multi-mode encoding support (CRF/ABR/CBR), intelligent content analysis with automatic profile 
selection, and comprehensive batch processing capabilities.

EXAMPLES:
  # Auto-selection with UUID output
  ffmpeg-encoder -i input.mkv -p auto -m crf

  # Specific profile with custom output  
  ffmpeg-encoder -i input.mkv -o output.mkv -p anime -m crf

  # With denoising
  ffmpeg-encoder -i input.mkv -p 4k_heavy_grain -m crf --denoise

  # Legacy interlaced footage with neural network deinterlacing
  ffmpeg-encoder -i legacy_footage.mkv -p classic_anime -m crf --deinterlace

  # Batch processing directory
  ffmpeg-encoder -i ~/Videos/Raw/ -p auto -m abr

  # With automatic crop detection
  ffmpeg-encoder -i input.mkv -p movie -m abr
")]
pub struct CliArgs {
    /// Input video file or directory (can be specified multiple times)
    #[arg(short, long, value_name = "PATH", action = clap::ArgAction::Append)]
    pub input: Vec<PathBuf>,

    /// Output file path (optional, auto-generates UUID-based name if not specified)
    #[arg(short, long, value_name = "PATH")]
    pub output: Option<PathBuf>,

    /// Encoding profile to use (use --list-profiles to see available profiles, or 'auto' for automatic selection)
    #[arg(short, long, default_value = "auto", value_name = "PROFILE")]
    pub profile: String,

    /// Video title for metadata
    #[arg(short, long, value_name = "TITLE")]
    pub title: Option<String>,

    /// Encoding mode: crf (quality), abr (average bitrate), cbr (constant bitrate)
    #[arg(short, long, default_value = "abr", value_parser = ["crf", "abr", "cbr"])]
    pub mode: String,

    /// Enable video denoising (hqdn3d=1:1:2:2)
    #[arg(long)]
    pub denoise: bool,

    /// Enable deinterlacing for interlaced content (NNEDI/yadif)
    #[arg(long)]
    pub deinterlace: bool,

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

    /// Stream selection profile to use (use --list-stream-profiles to see available profiles)
    #[arg(short = 's', long = "stream-selection-profile", value_name = "PROFILE")]
    pub stream_selection_profile: Option<String>,

    /// List all available stream selection profiles
    #[arg(long)]
    pub list_stream_profiles: bool,

    /// Show detailed information about a specific stream selection profile
    #[arg(long, value_name = "PROFILE")]
    pub show_stream_profile: Option<String>,
}

impl CliArgs {
    pub fn get_log_level<'a>(&self, config_level: &'a str) -> &'a str {
        if self.debug {
            "debug"
        } else {
            // Use config level if debug flag is not set
            config_level
        }
    }

    pub fn should_use_color(&self) -> bool {
        !self.no_color
    }

    pub fn is_info_command(&self) -> bool {
        self.list_profiles
            || self.show_profile.is_some()
            || self.list_stream_profiles
            || self.show_stream_profile.is_some()
            || self.validate_config
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
                    "At least one input path is required for encoding".to_string(),
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
        }

        // For validate-config command, we don't need to check if files exist
        // since the loading logic will handle fallbacks appropriately
        if !self.validate_config {
            // Only validate config file for commands that actually need it
            if (self.should_encode() || self.list_profiles || self.show_profile.is_some())
                && !self.config.exists()
            {
                let default_paths = [
                    std::path::Path::new("config.default.yaml"),
                    std::path::Path::new("./config/config.default.yaml"),
                ];

                let has_default = default_paths.iter().any(|p| p.exists());

                if !has_default {
                    return Err(crate::utils::Error::validation(format!(
                        "Configuration file does not exist: {} (and no config.default.yaml found)",
                        self.config.display()
                    )));
                }
            }
        }

        // Validate encoding mode
        if !["crf", "abr", "cbr"].contains(&self.mode.as_str()) {
            return Err(crate::utils::Error::validation(format!(
                "Invalid encoding mode: {} (must be crf, abr, or cbr)",
                self.mode
            )));
        }

        // Note: Profile validation is performed later after config is loaded
        // since profiles are defined dynamically in the configuration file

        Ok(())
    }

    pub fn print_help_topic(&self, topic: &str) {
        match topic.to_lowercase().as_str() {
            "profiles" => {
                println!("ENCODING PROFILES:\n");
                println!("Profiles are dynamically loaded from configuration file.");
                println!("Use --list-profiles to see all available profiles with details.");
                println!();
                println!("Special Profile:");
                println!("  auto            - Intelligent profile selection based on content analysis");
                println!();
                println!("To see detailed information about any profile:");
                println!("  ffmpeg-encoder --show-profile <PROFILE_NAME>");
            }
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
            }
            "examples" => {
                println!("USAGE EXAMPLES:\n");
                println!("Basic Usage:");
                println!("  ffmpeg-encoder -i input.mkv -p auto");
                println!("  ffmpeg-encoder -i input.mkv -o output.mkv -p anime -m crf");
                println!();
                println!("Advanced Options:");
                println!("  ffmpeg-encoder -i input.mkv -p 4k_heavy_grain --denoise");
                println!("  ffmpeg-encoder -i input.mkv -p movie --deinterlace -m abr");
                println!();
                println!("Batch Processing:");
                println!("  ffmpeg-encoder -i ~/Videos/Raw/ -p auto -m crf");
                println!();
                println!("Manual Overrides:");
                println!("  ffmpeg-encoder -i input.mkv -p anime --denoise");
                println!("  ffmpeg-encoder -i input.mkv -p auto -t \"Movie Title\"");
            }
            _ => {
                println!("Unknown help topic: {}", topic);
                println!("Available topics: profiles, modes, examples");
            }
        }
    }
}
