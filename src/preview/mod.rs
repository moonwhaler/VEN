use crate::{
    config::{Config, EncodingProfile, ProfileManager},
    utils::{ffmpeg::VideoMetadata, Error, FfmpegWrapper, Result},
};
use std::path::{Path, PathBuf};
use std::time::Duration;
use tracing::{info, warn};
use uuid::Uuid;

#[derive(Debug, Clone)]
pub enum PreviewMode {
    Image { timestamp: f64 },
    VideoSegment { start: f64, end: f64 },
}

#[derive(Debug, Clone)]
pub struct PreviewConfig {
    pub mode: PreviewMode,
    pub profile_names: Vec<String>,
}

#[derive(Debug)]
pub struct PreviewResult {
    pub profile_name: String,
    pub output_path: PathBuf,
    pub file_size: u64,
    pub encoding_duration: Duration,
}

pub struct PreviewProcessor<'a> {
    ffmpeg: &'a FfmpegWrapper,
    config: &'a Config,
    profile_manager: &'a ProfileManager,
    input_path: &'a Path,
    output_dir: PathBuf,
    preview_config: PreviewConfig,
    uuid: String,
}

impl<'a> PreviewProcessor<'a> {
    pub fn new(
        ffmpeg: &'a FfmpegWrapper,
        config: &'a Config,
        profile_manager: &'a ProfileManager,
        input_path: &'a Path,
        output_dir: Option<&Path>,
        preview_config: PreviewConfig,
    ) -> Self {
        let uuid = Uuid::new_v4().to_string();

        // Determine output directory: use provided output_dir, or input file's parent directory
        let output_dir = if let Some(dir) = output_dir {
            dir.to_path_buf()
        } else {
            input_path.parent()
                .map(|p| p.to_path_buf())
                .unwrap_or_else(|| PathBuf::from("."))
        };

        Self {
            ffmpeg,
            config,
            profile_manager,
            input_path,
            output_dir,
            preview_config,
            uuid,
        }
    }

    pub async fn generate_previews(&self) -> Result<Vec<PreviewResult>> {
        info!("Starting preview generation for: {}", self.input_path.display());
        info!("Using UUID: {}", self.uuid);

        match &self.preview_config.mode {
            PreviewMode::Image { timestamp } => {
                info!("Mode: Image generation at timestamp {}s", timestamp);
            }
            PreviewMode::VideoSegment { start, end } => {
                info!("Mode: Video segment from {}s to {}s (duration: {}s)", start, end, end - start);
            }
        }

        let metadata = self.ffmpeg.get_video_metadata(self.input_path).await?;
        self.validate_preview_parameters(&metadata)?;

        let mut results = Vec::new();

        for profile_name in &self.preview_config.profile_names {
            match self.profile_manager.get_profile(profile_name) {
                Some(profile) => {
                    info!("Generating preview with profile: {}", profile_name);
                    match self.generate_single_preview(profile, &metadata).await {
                        Ok(result) => {
                            info!(
                                "✓ Profile '{}': {} ({:.2} MB) - took {:.2}s",
                                result.profile_name,
                                result.output_path.display(),
                                result.file_size as f64 / 1_048_576.0,
                                result.encoding_duration.as_secs_f64()
                            );
                            results.push(result);
                        }
                        Err(e) => {
                            warn!("✗ Failed to generate preview for profile '{}': {}", profile_name, e);
                        }
                    }
                }
                None => {
                    warn!("Profile '{}' not found, skipping", profile_name);
                }
            }
        }

        if results.is_empty() {
            return Err(Error::encoding("No previews were successfully generated".to_string()));
        }

        info!("\n{}", self.generate_comparison_summary(&results));

        Ok(results)
    }

    fn validate_preview_parameters(&self, metadata: &VideoMetadata) -> Result<()> {
        match &self.preview_config.mode {
            PreviewMode::Image { timestamp } => {
                if *timestamp > metadata.duration {
                    return Err(Error::validation(format!(
                        "Preview timestamp ({:.2}s) exceeds video duration ({:.2}s)",
                        timestamp, metadata.duration
                    )));
                }
            }
            PreviewMode::VideoSegment { start, end } => {
                if *start > metadata.duration {
                    return Err(Error::validation(format!(
                        "Preview start time ({:.2}s) exceeds video duration ({:.2}s)",
                        start, metadata.duration
                    )));
                }
                if *end > metadata.duration {
                    return Err(Error::validation(format!(
                        "Preview end time ({:.2}s) exceeds video duration ({:.2}s)",
                        end, metadata.duration
                    )));
                }
            }
        }
        Ok(())
    }

    async fn generate_single_preview(
        &self,
        profile: &EncodingProfile,
        metadata: &VideoMetadata,
    ) -> Result<PreviewResult> {
        let start_time = std::time::Instant::now();

        let output_path = self.generate_preview_filename(&profile.name);

        match &self.preview_config.mode {
            PreviewMode::Image { timestamp } => {
                self.generate_image_preview(profile, *timestamp, &output_path, metadata)
                    .await?;
            }
            PreviewMode::VideoSegment { start, end } => {
                self.generate_video_preview(profile, *start, *end, &output_path, metadata)
                    .await?;
            }
        }

        let encoding_duration = start_time.elapsed();
        let file_size = std::fs::metadata(&output_path)?.len();

        Ok(PreviewResult {
            profile_name: profile.name.clone(),
            output_path,
            file_size,
            encoding_duration,
        })
    }

    fn generate_preview_filename(&self, profile_name: &str) -> PathBuf {
        let input_stem = self
            .input_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("preview");

        let time_info = match &self.preview_config.mode {
            PreviewMode::Image { timestamp } => format!("{}s", timestamp),
            PreviewMode::VideoSegment { start, end } => format!("{}-{}s", start, end),
        };

        let extension = match &self.preview_config.mode {
            PreviewMode::Image { .. } => "png",
            PreviewMode::VideoSegment { .. } => "mkv",
        };

        let filename = format!(
            "{}_preview_{}_{}_uuid-{}.{}",
            input_stem, profile_name, time_info, self.uuid, extension
        );

        self.output_dir.join(filename)
    }

    async fn generate_image_preview(
        &self,
        profile: &EncodingProfile,
        timestamp: f64,
        output_path: &Path,
        metadata: &VideoMetadata,
    ) -> Result<()> {
        // Two-step process to apply x265 profile settings to an image:
        // Step 1: Encode single frame with x265 + profile settings to temp MKV
        // Step 2: Extract that frame to PNG for viewing

        let temp_mkv = output_path.with_extension("temp.mkv");
        let final_png = output_path.with_extension("png");

        // Step 1: Encode with profile settings to temp MKV
        let x265_params = profile.build_x265_params_string_with_hdr_passthrough(
            None,
            Some(false),
            metadata.color_space.as_ref(),
            metadata.transfer_function.as_ref(),
            metadata.color_primaries.as_ref(),
            metadata.master_display.as_ref(),
            metadata.max_cll.as_ref(),
            false,
        );

        let mut cmd = tokio::process::Command::new(&self.config.tools.ffmpeg);
        cmd.arg("-ss")
            .arg(timestamp.to_string())
            .arg("-i")
            .arg(self.input_path)
            .arg("-vframes")
            .arg("1")
            .arg("-c:v")
            .arg("libx265")
            .arg("-x265-params")
            .arg(&x265_params)
            .arg("-crf")
            .arg(profile.base_crf.to_string())
            .arg("-preset")
            .arg(profile.x265_params.get("preset")
                .map(|s| s.as_str())
                .unwrap_or("medium"))
            .arg("-an")
            .arg("-y")
            .arg(&temp_mkv);

        let output = cmd.output().await?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let _ = tokio::fs::remove_file(&temp_mkv).await;
            return Err(Error::ffmpeg(format!(
                "FFmpeg failed to encode preview frame: {}",
                stderr
            )));
        }

        // Step 2: Extract encoded frame to PNG
        let mut cmd2 = tokio::process::Command::new(&self.config.tools.ffmpeg);
        cmd2.arg("-i")
            .arg(&temp_mkv)
            .arg("-vframes")
            .arg("1")
            .arg("-c:v")
            .arg("png")
            .arg("-y")
            .arg(&final_png);

        let output2 = cmd2.output().await?;

        // Clean up temp file
        let _ = tokio::fs::remove_file(&temp_mkv).await;

        if !output2.status.success() {
            let stderr = String::from_utf8_lossy(&output2.stderr);
            return Err(Error::ffmpeg(format!(
                "FFmpeg failed to extract PNG: {}",
                stderr
            )));
        }

        Ok(())
    }

    async fn generate_video_preview(
        &self,
        profile: &EncodingProfile,
        start: f64,
        end: f64,
        output_path: &Path,
        metadata: &VideoMetadata,
    ) -> Result<()> {
        let x265_params = profile.build_x265_params_string_with_hdr_passthrough(
            None,
            Some(false),
            metadata.color_space.as_ref(),
            metadata.transfer_function.as_ref(),
            metadata.color_primaries.as_ref(),
            metadata.master_display.as_ref(),
            metadata.max_cll.as_ref(),
            false,
        );

        let mut cmd = tokio::process::Command::new(&self.config.tools.ffmpeg);
        cmd.arg("-ss")
            .arg(start.to_string())
            .arg("-to")
            .arg(end.to_string())
            .arg("-i")
            .arg(self.input_path)
            .arg("-c:v")
            .arg("libx265")
            .arg("-x265-params")
            .arg(&x265_params)
            .arg("-crf")
            .arg(profile.base_crf.to_string())
            .arg("-preset")
            .arg(profile.x265_params.get("preset")
                .map(|s| s.as_str())
                .unwrap_or("medium"))
            .arg("-c:a")
            .arg("copy")
            .arg("-y")
            .arg(output_path);

        let output = cmd.output().await?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(Error::ffmpeg(format!(
                "FFmpeg failed to generate video preview: {}",
                stderr
            )));
        }

        Ok(())
    }

    fn generate_comparison_summary(&self, results: &[PreviewResult]) -> String {
        let mut summary = "=".repeat(80);
        summary.push_str("\nPREVIEW COMPARISON RESULTS\n");
        summary.push_str(&"=".repeat(80));
        summary.push_str(&format!("\nInput: {}\n", self.input_path.display()));
        summary.push_str(&format!("UUID: {}\n", self.uuid));

        match &self.preview_config.mode {
            PreviewMode::Image { timestamp } => {
                summary.push_str(&format!("Mode: Image at {}s\n", timestamp));
            }
            PreviewMode::VideoSegment { start, end } => {
                summary.push_str(&format!(
                    "Mode: Video segment from {}s to {}s ({}s duration)\n",
                    start,
                    end,
                    end - start
                ));
            }
        }

        summary.push_str(&"=".repeat(80));
        summary.push_str("\n\n");
        summary.push_str(&format!(
            "{:<25} {:>12} {:>12} {:>20}\n",
            "Profile", "Size (MB)", "Time (s)", "Output File"
        ));
        summary.push_str(&"-".repeat(80));
        summary.push('\n');

        for result in results {
            let size_mb = result.file_size as f64 / 1_048_576.0;
            let time_s = result.encoding_duration.as_secs_f64();
            let filename = result
                .output_path
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("unknown");

            summary.push_str(&format!(
                "{:<25} {:>12.2} {:>12.2} {:>20}\n",
                result.profile_name, size_mb, time_s, filename
            ));
        }

        summary.push_str(&"=".repeat(80));
        summary.push('\n');

        summary
    }
}
