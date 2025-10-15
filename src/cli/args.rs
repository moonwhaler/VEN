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

    /// Enable preview mode (generates previews instead of full encoding)
    #[arg(long)]
    pub preview: bool,

    /// Preview timestamp in seconds (for single frame image generation)
    #[arg(long, value_name = "SECONDS")]
    pub preview_time: Option<f64>,

    /// Preview time range in format "START-END" (for video segment encoding, e.g., "10-20")
    #[arg(long, value_name = "START-END")]
    pub preview_range: Option<String>,

    /// Preview profile group to use (from config's preview_profiles section)
    #[arg(long, value_name = "NAME")]
    pub preview_profile: Option<String>,

    /// List available preview profile groups
    #[arg(long)]
    pub list_preview_profiles: bool,
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

    pub fn is_info_command(&self) -> bool {
        self.list_profiles
            || self.show_profile.is_some()
            || self.list_stream_profiles
            || self.show_stream_profile.is_some()
            || self.list_preview_profiles
            || self.validate_config
    }

    pub fn should_encode(&self) -> bool {
        !self.is_info_command() && !self.input.is_empty() && !self.preview
    }

    pub fn should_preview(&self) -> bool {
        self.preview && !self.input.is_empty()
    }

    pub fn validate(&self) -> Result<()> {
        // Validate preview mode parameters
        if self.preview {
            if self.input.is_empty() {
                return Err(crate::utils::Error::validation(
                    "At least one input path is required for preview mode".to_string(),
                ));
            }

            // Must specify either preview_time OR preview_range
            if self.preview_time.is_none() && self.preview_range.is_none() {
                return Err(crate::utils::Error::validation(
                    "Preview mode requires either --preview-time or --preview-range".to_string(),
                ));
            }

            // Cannot specify both
            if self.preview_time.is_some() && self.preview_range.is_some() {
                return Err(crate::utils::Error::validation(
                    "Cannot use both --preview-time and --preview-range simultaneously".to_string(),
                ));
            }

            // Validate preview_time is positive
            if let Some(time) = self.preview_time {
                if time < 0.0 {
                    return Err(crate::utils::Error::validation(
                        "Preview time must be a positive number".to_string(),
                    ));
                }
            }

            // Validate preview_range format
            if let Some(range) = &self.preview_range {
                self.validate_preview_range(range)?;
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

    fn validate_preview_range(&self, range: &str) -> Result<()> {
        let parts: Vec<&str> = range.split('-').collect();
        if parts.len() != 2 {
            return Err(crate::utils::Error::validation(
                "Preview range must be in format 'START-END' (e.g., '10-20')".to_string(),
            ));
        }

        let start: f64 = parts[0].parse().map_err(|_| {
            crate::utils::Error::validation(format!(
                "Invalid start time in preview range: '{}'",
                parts[0]
            ))
        })?;

        let end: f64 = parts[1].parse().map_err(|_| {
            crate::utils::Error::validation(format!(
                "Invalid end time in preview range: '{}'",
                parts[1]
            ))
        })?;

        if start < 0.0 || end < 0.0 {
            return Err(crate::utils::Error::validation(
                "Preview range times must be positive numbers".to_string(),
            ));
        }

        if start >= end {
            return Err(crate::utils::Error::validation(
                "Preview range start time must be less than end time".to_string(),
            ));
        }

        Ok(())
    }

    pub fn parse_preview_range(&self) -> Option<(f64, f64)> {
        self.preview_range.as_ref().and_then(|range| {
            let parts: Vec<&str> = range.split('-').collect();
            if parts.len() == 2 {
                if let (Ok(start), Ok(end)) = (parts[0].parse::<f64>(), parts[1].parse::<f64>()) {
                    return Some((start, end));
                }
            }
            None
        })
    }
}
