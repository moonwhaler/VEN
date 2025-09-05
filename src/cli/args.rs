use clap::{Parser, Subcommand, Args};
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
#[command(name = "ffmpeg-encoder")]
#[command(about = "Modern Rust-based FFmpeg video encoding automation")]
pub struct CliArgs {
    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// List available encoding profiles
    ListProfiles,
    /// Show detailed information about a specific profile
    ShowProfile {
        /// Profile name to show
        name: String,
    },
    /// Validate configuration file
    ValidateConfig {
        /// Path to configuration file
        #[arg(short, long, default_value = "config.yaml")]
        config: PathBuf,
    },
    /// Encode video files
    Encode(EncodingCommand),
}

#[derive(Args, Debug)]
pub struct EncodingCommand {
    /// Input video file or directory
    #[arg(short, long)]
    pub input: PathBuf,

    /// Output file path (optional, auto-generates UUID-based name if not specified)
    #[arg(short, long)]
    pub output: Option<PathBuf>,

    /// Encoding profile to use
    #[arg(short, long, default_value = "auto")]
    pub profile: String,

    /// Video title for metadata
    #[arg(short, long)]
    pub title: Option<String>,

    /// Encoding mode: crf (quality), abr (average bitrate), cbr (constant bitrate)
    #[arg(short, long, default_value = "abr", value_parser = ["crf", "abr", "cbr"])]
    pub mode: String,

    /// Manual crop values in format width:height:x:y
    #[arg(short, long)]
    pub crop: Option<String>,

    /// Scale video to specified resolution (widthxheight, -1 for auto)
    #[arg(short, long)]
    pub scale: Option<String>,

    /// Enable complexity analysis for better parameter optimization
    #[arg(long)]
    pub use_complexity: bool,

    /// Enable video denoising
    #[arg(long)]
    pub denoise: bool,

    /// Enable deinterlacing for interlaced content
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
    #[arg(long, default_value = "config.yaml")]
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
}

impl CliArgs {
    pub fn is_command(&self) -> bool {
        match &self.command {
            Some(Commands::Encode(_)) | None => false,
            _ => true,
        }
    }

    pub fn should_encode(&self) -> bool {
        match &self.command {
            Some(Commands::Encode(_)) | None => true,
            _ => false,
        }
    }

    pub fn get_encoding_command(&self) -> Option<&EncodingCommand> {
        match &self.command {
            Some(Commands::Encode(cmd)) => Some(cmd),
            None => None,
            _ => None,
        }
    }
}

impl EncodingCommand {
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

    pub fn validate(&self) -> crate::utils::Result<()> {
        if self.should_encode() && !self.input.exists() {
            return Err(crate::utils::Error::validation(format!(
                "Input path does not exist: {}",
                self.input.display()
            )));
        }

        if !["crf", "abr", "cbr"].contains(&self.mode.as_str()) {
            return Err(crate::utils::Error::validation(format!(
                "Invalid encoding mode: {} (must be crf, abr, or cbr)",
                self.mode
            )));
        }

        if let Some(crop) = &self.crop {
            if !self.is_valid_crop_format(crop) {
                return Err(crate::utils::Error::validation(format!(
                    "Invalid crop format: {} (expected format: width:height:x:y)",
                    crop
                )));
            }
        }

        if let Some(scale) = &self.scale {
            if !self.is_valid_scale_format(scale) {
                return Err(crate::utils::Error::validation(format!(
                    "Invalid scale format: {} (expected format: widthxheight)",
                    scale
                )));
            }
        }

        Ok(())
    }

    fn should_encode(&self) -> bool {
        !self.input.as_os_str().is_empty()
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[test]
    fn test_cli_args_parsing() {
        let args = CliArgs::parse_from(&[
            "ffmpeg-encoder",
            "-i", "input.mkv",
            "-o", "output.mkv",
            "-p", "anime",
            "-m", "crf",
            "--use-complexity",
            "--denoise",
        ]);

        assert!(!args.is_command());
        assert!(args.should_encode());
        assert_eq!(args.encoding.input, PathBuf::from("input.mkv"));
        assert_eq!(args.encoding.output, Some(PathBuf::from("output.mkv")));
        assert_eq!(args.encoding.profile, "anime");
        assert_eq!(args.encoding.mode, "crf");
        assert!(args.encoding.use_complexity);
        assert!(args.encoding.denoise);
    }

    #[test]
    fn test_cli_args_list_profiles_command() {
        let args = CliArgs::parse_from(&[
            "ffmpeg-encoder",
            "list-profiles",
        ]);

        assert!(args.is_command());
        assert!(!args.should_encode());
        matches!(args.command, Some(Commands::ListProfiles));
    }

    #[test]
    fn test_cli_args_show_profile_command() {
        let args = CliArgs::parse_from(&[
            "ffmpeg-encoder",
            "show-profile",
            "anime",
        ]);

        assert!(args.is_command());
        matches!(args.command, Some(Commands::ShowProfile { name }) if name == "anime");
    }

    #[test]
    fn test_encoding_command_log_level() {
        let mut cmd = EncodingCommand {
            input: PathBuf::from("test.mkv"),
            debug: true,
            verbose: false,
            ..Default::default()
        };
        assert_eq!(cmd.get_log_level(), "debug");

        cmd.debug = false;
        cmd.verbose = true;
        assert_eq!(cmd.get_log_level(), "info");

        cmd.verbose = false;
        assert_eq!(cmd.get_log_level(), "warn");
    }
}

impl Default for EncodingCommand {
    fn default() -> Self {
        Self {
            input: PathBuf::new(),
            output: None,
            profile: "auto".to_string(),
            title: None,
            mode: "abr".to_string(),
            crop: None,
            scale: None,
            use_complexity: false,
            denoise: false,
            deinterlace: false,
            hardware: false,
            web_search_force: false,
            no_web_search: false,
            config: PathBuf::from("config.yaml"),
            verbose: false,
            debug: false,
            no_color: false,
        }
    }
}