use chrono::Utc;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tracing::Level;
use tracing_subscriber::{
    fmt::{self, time::ChronoUtc},
    layer::SubscriberExt,
    util::SubscriberInitExt,
    EnvFilter, Layer,
};

pub fn setup_logging(
    level: &str,
    show_timestamps: bool,
    _colored: bool,
) -> crate::utils::Result<()> {
    let level = match level.to_lowercase().as_str() {
        "trace" => Level::TRACE,
        "debug" => Level::DEBUG,
        "info" => Level::INFO,
        "warn" => Level::WARN,
        "error" => Level::ERROR,
        _ => Level::INFO,
    };

    let env_filter = EnvFilter::builder()
        .with_default_directive(level.into())
        .from_env_lossy();

    let fmt_layer = fmt::layer()
        .with_target(false)
        .with_level(true)
        .with_ansi(false) // Disable ANSI formatting to remove emojis
        .compact();

    let fmt_layer = if show_timestamps {
        fmt_layer.with_timer(ChronoUtc::rfc_3339()).boxed()
    } else {
        fmt_layer.without_time().boxed()
    };

    tracing_subscriber::registry()
        .with(env_filter)
        .with(fmt_layer)
        .init();

    Ok(())
}

pub fn log_encoding_start(input: &str, output: &str, profile: &str, mode: &str) {
    tracing::info!(
        "Starting encoding: {} -> {} (profile: {}, mode: {})",
        input,
        output,
        profile,
        mode
    );
}

pub fn log_encoding_complete(duration: std::time::Duration, output_size: u64) {
    tracing::info!(
        "Encoding completed in {:.2}s, output size: {:.2} MB",
        duration.as_secs_f64(),
        output_size as f64 / 1_048_576.0
    );
}

pub fn log_analysis_result(content_type: &str, grain_level: u8) {
    tracing::info!("Analysis: type={}, grain={}", content_type, grain_level);
}

pub fn log_crop_detection(crop_values: &str) {
    tracing::info!("Crop detected: {}", crop_values);
}

pub fn log_profile_selection(profile: &str, reason: &str) {
    tracing::info!("Profile selected: {} ({})", profile, reason);
}

pub struct FileLogger {
    writer: Arc<Mutex<BufWriter<File>>>,
    log_path: PathBuf,
}

impl FileLogger {
    pub fn new<P: AsRef<Path>>(output_path: P) -> crate::utils::Result<Self> {
        let output_path = output_path.as_ref();
        let log_path = output_path.with_extension("log");

        let file = File::create(&log_path)?;
        let writer = Arc::new(Mutex::new(BufWriter::new(file)));

        Ok(Self { writer, log_path })
    }

    #[allow(clippy::too_many_arguments)]
    pub fn log_encoding_settings(
        &self,
        input_path: &Path,
        output_path: &Path,
        profile_name: &str,
        profile_settings: &crate::config::EncodingProfile,
        mode: &str,
        adaptive_crf: f32,
        adaptive_bitrate: u32,
        filter_chain: Option<&str>,
        stream_mapping: &str,
    ) -> crate::utils::Result<()> {
        let timestamp = Utc::now().format("%Y-%m-%d %H:%M:%S UTC");

        let mut writer = self.writer.lock().unwrap();

        writeln!(writer, "FFmpeg Encoder - Encoding Log")?;
        writeln!(writer, "Generated: {}", timestamp)?;
        writeln!(writer, "========================================\n")?;

        writeln!(writer, "INPUT/OUTPUT:")?;
        writeln!(writer, "  Input:  {}", input_path.display())?;
        writeln!(writer, "  Output: {}", output_path.display())?;
        writeln!(writer)?;

        writeln!(writer, "ENCODING SETTINGS:")?;
        writeln!(writer, "  Mode: {}", mode.to_uppercase())?;
        writeln!(
            writer,
            "  Profile: {} - {}",
            profile_name, profile_settings.title
        )?;
        writeln!(
            writer,
            "  Content Type: {:?}",
            profile_settings.content_type
        )?;
        writeln!(writer, "  Adaptive CRF: {}", adaptive_crf)?;
        writeln!(writer, "  Adaptive Bitrate: {} kbps", adaptive_bitrate)?;
        writeln!(writer)?;

        if let Some(filters) = filter_chain {
            writeln!(writer, "VIDEO FILTERS:")?;
            writeln!(writer, "  {}", filters)?;
            writeln!(writer)?;
        }

        writeln!(writer, "STREAM MAPPING:")?;
        writeln!(writer, "  {}", stream_mapping)?;
        writeln!(writer)?;

        writeln!(writer, "x265 PARAMETERS:")?;
        for (key, value) in &profile_settings.x265_params {
            writeln!(writer, "  {}: {}", key, value)?;
        }
        writeln!(writer)?;

        writer.flush()?;
        Ok(())
    }

    pub fn log_analysis_results(
        &self,
        metadata: &crate::utils::ffmpeg::VideoMetadata,
        grain_level: Option<u8>,
        content_analysis: Option<&crate::content_manager::ContentAnalysisResult>,
    ) -> crate::utils::Result<()> {
        let mut writer = self.writer.lock().unwrap();

        writeln!(writer, "VIDEO ANALYSIS:")?;
        writeln!(
            writer,
            "  Resolution: {}x{}",
            metadata.width, metadata.height
        )?;
        writeln!(writer, "  Duration: {:.2}s", metadata.duration)?;
        writeln!(writer, "  Framerate: {:.2} fps", metadata.fps)?;
        writeln!(
            writer,
            "  Codec: {}",
            metadata.codec.as_deref().unwrap_or("Unknown")
        )?;
        if let Some(bitrate) = metadata.bitrate {
            writeln!(writer, "  Bitrate: {} kbps", bitrate)?;
        }
        writeln!(
            writer,
            "  HDR: {}",
            if metadata.is_hdr { "Yes" } else { "No" }
        )?;

        if let Some(grain) = grain_level {
            writeln!(writer, "  Grain Level: {}", grain)?;
        }

        // Enhanced HDR/DV analysis logging
        if let Some(analysis) = content_analysis {
            writeln!(writer)?;
            writeln!(writer, "HDR/DOLBY VISION ANALYSIS:")?;

            match &analysis.recommended_approach {
                crate::content_manager::ContentEncodingApproach::SDR => {
                    writeln!(writer, "  Content Type: SDR (Standard Dynamic Range)")?;
                    writeln!(writer, "  HDR Format: None")?;
                }
                crate::content_manager::ContentEncodingApproach::HDR(hdr_result) => {
                    writeln!(writer, "  Content Type: HDR (High Dynamic Range)")?;
                    writeln!(writer, "  HDR Format: {:?}", hdr_result.metadata.format)?;
                    writeln!(
                        writer,
                        "  Detection Confidence: {:.1}%",
                        hdr_result.confidence_score * 100.0
                    )?;

                    // Color space information
                    if let Some(ref cs) = hdr_result.metadata.raw_color_space {
                        writeln!(writer, "  Color Space: {}", cs)?;
                    }
                    if let Some(ref tf) = hdr_result.metadata.raw_transfer {
                        writeln!(writer, "  Transfer Function: {}", tf)?;
                    }
                    if let Some(ref cp) = hdr_result.metadata.raw_primaries {
                        writeln!(writer, "  Color Primaries: {}", cp)?;
                    }

                    // Mastering display metadata
                    if let Some(ref master_display) = hdr_result.metadata.master_display {
                        writeln!(writer, "  Mastering Display Metadata:")?;
                        writeln!(
                            writer,
                            "    Red Primary: ({:.4}, {:.4})",
                            master_display.red_primary.0, master_display.red_primary.1
                        )?;
                        writeln!(
                            writer,
                            "    Green Primary: ({:.4}, {:.4})",
                            master_display.green_primary.0, master_display.green_primary.1
                        )?;
                        writeln!(
                            writer,
                            "    Blue Primary: ({:.4}, {:.4})",
                            master_display.blue_primary.0, master_display.blue_primary.1
                        )?;
                        writeln!(
                            writer,
                            "    White Point: ({:.4}, {:.4})",
                            master_display.white_point.0, master_display.white_point.1
                        )?;
                        writeln!(
                            writer,
                            "    Max Luminance: {} nits",
                            master_display.max_luminance
                        )?;
                        writeln!(
                            writer,
                            "    Min Luminance: {:.4} nits",
                            master_display.min_luminance
                        )?;
                    }

                    // Content light level information
                    if let Some(ref cll) = hdr_result.metadata.content_light_level {
                        writeln!(writer, "  Content Light Level:")?;
                        writeln!(writer, "    Max CLL: {} nits", cll.max_cll)?;
                        writeln!(writer, "    Max FALL: {} nits", cll.max_fall)?;
                    }
                }
                crate::content_manager::ContentEncodingApproach::DolbyVision(dv_info) => {
                    writeln!(writer, "  Content Type: Dolby Vision")?;
                    writeln!(
                        writer,
                        "  Dolby Vision Profile: {}",
                        dv_info.profile.as_str()
                    )?;
                    writeln!(
                        writer,
                        "  Profile Description: {}",
                        Self::get_dv_profile_description(&dv_info.profile)
                    )?;
                    writeln!(
                        writer,
                        "  RPU Present: {}",
                        if dv_info.rpu_present { "Yes" } else { "No" }
                    )?;
                    writeln!(
                        writer,
                        "  Has Enhancement Layer: {}",
                        if dv_info.has_enhancement_layer {
                            "Yes"
                        } else {
                            "No"
                        }
                    )?;
                    writeln!(
                        writer,
                        "  EL Present: {}",
                        if dv_info.el_present { "Yes" } else { "No" }
                    )?;
                    writeln!(
                        writer,
                        "  HDR10 Compatible: {}",
                        if dv_info.profile.supports_hdr10_compatibility() {
                            "Yes"
                        } else {
                            "No"
                        }
                    )?;
                    writeln!(
                        writer,
                        "  Dual Layer: {}",
                        if dv_info.profile.is_dual_layer() {
                            "Yes"
                        } else {
                            "No"
                        }
                    )?;

                    if let Some(bl_compatible_id) = dv_info.bl_compatible_id {
                        writeln!(writer, "  BL Compatible ID: {}", bl_compatible_id)?;
                    }

                    if let Some(ref codec_profile) = dv_info.codec_profile {
                        writeln!(writer, "  Codec Profile: {}", codec_profile)?;
                    }
                }
                crate::content_manager::ContentEncodingApproach::DolbyVisionWithHDR10Plus(
                    dv_info,
                    hdr_result,
                ) => {
                    writeln!(
                        writer,
                        "  Content Type: Dual Format (Dolby Vision + HDR10+)"
                    )?;

                    // Dolby Vision information
                    writeln!(writer, "  Dolby Vision:")?;
                    writeln!(writer, "    Profile: {}", dv_info.profile.as_str())?;
                    writeln!(
                        writer,
                        "    Profile Description: {}",
                        Self::get_dv_profile_description(&dv_info.profile)
                    )?;
                    writeln!(
                        writer,
                        "    RPU Present: {}",
                        if dv_info.rpu_present { "Yes" } else { "No" }
                    )?;
                    writeln!(
                        writer,
                        "    HDR10 Compatible: {}",
                        if dv_info.profile.supports_hdr10_compatibility() {
                            "Yes"
                        } else {
                            "No"
                        }
                    )?;
                    writeln!(
                        writer,
                        "    EL Present: {}",
                        if dv_info.el_present { "Yes" } else { "No" }
                    )?;

                    // HDR10+ information
                    writeln!(writer, "  HDR10+ Format: {:?}", hdr_result.metadata.format)?;
                    writeln!(
                        writer,
                        "  HDR Detection Confidence: {:.1}%",
                        hdr_result.confidence_score * 100.0
                    )?;

                    if let Some(ref master_display) = hdr_result.metadata.master_display {
                        writeln!(
                            writer,
                            "  Max Luminance: {} nits",
                            master_display.max_luminance
                        )?;
                        writeln!(
                            writer,
                            "  Min Luminance: {:.4} nits",
                            master_display.min_luminance
                        )?;
                    }

                    if let Some(ref cll) = hdr_result.metadata.content_light_level {
                        writeln!(writer, "  Max CLL: {} nits", cll.max_cll)?;
                        writeln!(writer, "  Max FALL: {} nits", cll.max_fall)?;
                    }
                }
            }

            // Encoding adjustments section
            writeln!(writer)?;
            writeln!(writer, "CONTENT-BASED ENCODING ADJUSTMENTS:")?;
            writeln!(
                writer,
                "  CRF Adjustment: {:+.1}",
                analysis.encoding_adjustments.crf_adjustment
            )?;
            writeln!(
                writer,
                "  Bitrate Multiplier: {:.2}x",
                analysis.encoding_adjustments.bitrate_multiplier
            )?;
            writeln!(
                writer,
                "  Encoding Complexity: {:.2}x",
                analysis.encoding_adjustments.encoding_complexity
            )?;
            writeln!(
                writer,
                "  Recommended CRF Range: {:.1}-{:.1}",
                analysis.encoding_adjustments.recommended_crf_range.0,
                analysis.encoding_adjustments.recommended_crf_range.1
            )?;

            if analysis.encoding_adjustments.requires_vbv {
                writeln!(writer, "  VBV Required: Yes")?;
                if let Some(bufsize) = analysis.encoding_adjustments.vbv_bufsize {
                    writeln!(writer, "  VBV Buffer Size: {} kbps", bufsize)?;
                }
                if let Some(maxrate) = analysis.encoding_adjustments.vbv_maxrate {
                    writeln!(writer, "  VBV Max Rate: {} kbps", maxrate)?;
                }
            } else {
                writeln!(writer, "  VBV Required: No")?;
            }

            // HDR10+ specific information
            if let Some(ref hdr10plus_result) = analysis.hdr10_plus {
                writeln!(writer)?;
                writeln!(writer, "HDR10+ DYNAMIC METADATA:")?;
                writeln!(
                    writer,
                    "  Extraction Successful: {}",
                    if hdr10plus_result.extraction_successful {
                        "Yes"
                    } else {
                        "No"
                    }
                )?;
                writeln!(
                    writer,
                    "  Metadata File: {}",
                    hdr10plus_result.metadata_file.display()
                )?;
                writeln!(writer, "  Curve Count: {}", hdr10plus_result.curve_count)?;
                writeln!(writer, "  Scene Count: {}", hdr10plus_result.scene_count)?;

                if let Some(file_size) = hdr10plus_result.file_size {
                    writeln!(writer, "  Metadata File Size: {} bytes", file_size)?;
                }

                // Access metadata fields directly since it's not optional
                let metadata = &hdr10plus_result.metadata;
                writeln!(writer, "  Metadata Version: {}", metadata.version)?;
                writeln!(writer, "  Frame Count: {}", metadata.num_frames)?;

                if let Some(ref source) = metadata.source {
                    writeln!(
                        writer,
                        "  Source: {}",
                        source.filename.as_deref().unwrap_or("Unknown")
                    )?;
                    if let Some(resolution) = &source.resolution {
                        writeln!(writer, "  Source Resolution: {}", resolution)?;
                    }
                    if let Some(frame_rate) = source.frame_rate {
                        writeln!(writer, "  Source Frame Rate: {:.2} fps", frame_rate)?;
                    }
                }

                // Scene information
                if let Some(ref scene_info) = metadata.scene_info {
                    writeln!(writer, "  Scene Count: {}", scene_info.len())?;
                    for (i, scene) in scene_info.iter().enumerate().take(3) {
                        // Limit to first 3
                        writeln!(
                            writer,
                            "    Scene {}: Frames {}-{}, Avg MaxRGB: {:.2}",
                            i + 1,
                            scene.first_frame,
                            scene.last_frame,
                            scene.average_maxrgb.unwrap_or(0.0)
                        )?;
                    }
                    if scene_info.len() > 3 {
                        writeln!(writer, "    ... and {} more scenes", scene_info.len() - 3)?;
                    }
                }

                // Frame metadata summary
                if !metadata.frames.is_empty() {
                    writeln!(
                        writer,
                        "  Frame Metadata: {} frames with tone mapping data",
                        metadata.frames.len()
                    )?;
                    if let Some(first_frame) = metadata.frames.first() {
                        if let Some(app_version) = first_frame.application_version {
                            writeln!(writer, "  Application Version: {}", app_version)?;
                        }
                        if let Some(target_lum) =
                            first_frame.targeted_system_display_maximum_luminance
                        {
                            writeln!(writer, "  Target Max Luminance: {:.1} nits", target_lum)?;
                        }
                    }
                }
            }
        }

        writeln!(writer)?;
        writer.flush()?;
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub fn log_crop_detection_results(
        &self,
        enabled: bool,
        sample_count: u32,
        sample_timestamps: &[f64],
        crop_result: Option<&str>,
        detection_method: &str,
        sdr_limit: u32,
        hdr_limit: u32,
        is_hdr: bool,
    ) -> crate::utils::Result<()> {
        let mut writer = self.writer.lock().unwrap();

        writeln!(writer, "CROP DETECTION:")?;
        writeln!(writer, "  Enabled: {}", if enabled { "Yes" } else { "No" })?;

        if !enabled {
            writeln!(writer)?;
            writer.flush()?;
            return Ok(());
        }

        writeln!(writer, "  Sample Count: {}", sample_count)?;

        // Format timestamps for display
        let timestamp_display =
            if sample_timestamps.len() == 1 && (sample_timestamps[0] + 1.0).abs() < f64::EPSILON {
                "Manual Override".to_string()
            } else {
                sample_timestamps
                    .iter()
                    .map(|&t| format!("{:.1}s", t))
                    .collect::<Vec<_>>()
                    .join(", ")
            };
        writeln!(writer, "  Sample Timestamps: {}", timestamp_display)?;
        writeln!(writer, "  Detection Method: {}", detection_method)?;

        let used_limit = if is_hdr { hdr_limit } else { sdr_limit };
        writeln!(
            writer,
            "  Crop Threshold: {} ({} content)",
            used_limit,
            if is_hdr { "HDR" } else { "SDR" }
        )?;

        match crop_result {
            Some(crop) => {
                writeln!(writer, "  Result: CROP DETECTED")?;
                writeln!(writer, "  Crop Values: {}", crop)?;

                // Parse and calculate crop statistics
                if let Some(stats) = self.parse_crop_statistics(crop) {
                    writeln!(writer, "  Original Resolution: {}x{}", stats.0, stats.1)?;
                    writeln!(writer, "  Cropped Resolution: {}x{}", stats.2, stats.3)?;
                    writeln!(writer, "  Pixels Removed: {:.1}%", stats.4)?;
                }
            }
            None => {
                writeln!(writer, "  Result: NO CROP DETECTED")?;
                writeln!(
                    writer,
                    "  Reason: No consistent black bars found across sample points"
                )?;
            }
        }

        writeln!(writer)?;
        writer.flush()?;
        Ok(())
    }

    fn parse_crop_statistics(&self, crop_str: &str) -> Option<(u32, u32, u32, u32, f32)> {
        // Parse crop string like "1920:800:0:140"
        let parts: Vec<&str> = crop_str.split(':').collect();
        if parts.len() != 4 {
            return None;
        }

        let width: u32 = parts[0].parse().ok()?;
        let height: u32 = parts[1].parse().ok()?;
        let _x: u32 = parts[2].parse().ok()?;
        let _y: u32 = parts[3].parse().ok()?;

        // For statistics, we need to calculate based on common resolutions
        // This is a simple heuristic - in practice, we'd pass the original resolution
        let (orig_width, orig_height) = if width <= 1920 && height <= 1080 {
            (1920u32, 1080u32)
        } else if width <= 3840 && height <= 2160 {
            (3840u32, 2160u32)
        } else {
            // Estimate based on crop dimensions
            (width, height + 280) // Common letterbox height
        };

        let orig_pixels = (orig_width * orig_height) as f32;
        let crop_pixels = (width * height) as f32;
        let removed_percent = ((orig_pixels - crop_pixels) / orig_pixels) * 100.0;

        Some((orig_width, orig_height, width, height, removed_percent))
    }

    pub fn log_encoding_progress(&self, message: &str) -> crate::utils::Result<()> {
        let mut writer = self.writer.lock().unwrap();
        let timestamp = Utc::now().format("%H:%M:%S");
        writeln!(writer, "[{}] {}", timestamp, message)?;
        writer.flush()?;
        Ok(())
    }

    pub fn log_encoding_complete(
        &self,
        success: bool,
        duration: std::time::Duration,
        output_size: Option<u64>,
        exit_code: Option<i32>,
    ) -> crate::utils::Result<()> {
        let mut writer = self.writer.lock().unwrap();

        writeln!(writer, "ENCODING RESULT:")?;
        writeln!(
            writer,
            "  Status: {}",
            if success { "SUCCESS" } else { "FAILED" }
        )?;
        writeln!(writer, "  Duration: {:.2}s", duration.as_secs_f64())?;

        if let Some(size) = output_size {
            writeln!(writer, "  Output Size: {:.2} MB", size as f64 / 1_048_576.0)?;
        }

        if let Some(code) = exit_code {
            writeln!(writer, "  Exit Code: {}", code)?;
        }

        let timestamp = Utc::now().format("%Y-%m-%d %H:%M:%S UTC");
        writeln!(writer, "  Completed: {}", timestamp)?;
        writeln!(writer)?;

        writer.flush()?;
        Ok(())
    }

    pub fn log_ffmpeg_command(
        &self,
        ffmpeg_path: &str,
        args: &[String],
    ) -> crate::utils::Result<()> {
        let mut writer = self.writer.lock().unwrap();

        writeln!(writer, "RAW FFMPEG COMMAND:")?;

        // Build the complete command exactly as it will be executed
        // start_encoding() adds -y at the beginning, so we replicate that here
        let mut full_command = vec![ffmpeg_path.to_string(), "-y".to_string()];
        full_command.extend_from_slice(args);

        // Write as a single line that can be copy-pasted
        writeln!(writer, "  {}", full_command.join(" "))?;
        writeln!(writer)?;

        writer.flush()?;
        Ok(())
    }

    pub fn get_log_path(&self) -> &Path {
        &self.log_path
    }

    /// Helper method to get Dolby Vision profile descriptions
    fn get_dv_profile_description(
        profile: &crate::analysis::dolby_vision::DolbyVisionProfile,
    ) -> &'static str {
        match profile {
            crate::analysis::dolby_vision::DolbyVisionProfile::None => "Not Dolby Vision",
            crate::analysis::dolby_vision::DolbyVisionProfile::Profile5 => "Single-layer DV only",
            crate::analysis::dolby_vision::DolbyVisionProfile::Profile7 => {
                "Dual-layer (BL + EL + RPU)"
            }
            crate::analysis::dolby_vision::DolbyVisionProfile::Profile81 => {
                "Single-layer with HDR10 compatibility"
            }
            crate::analysis::dolby_vision::DolbyVisionProfile::Profile82 => {
                "Single-layer with SDR compatibility"
            }
            crate::analysis::dolby_vision::DolbyVisionProfile::Profile84 => {
                "HDMI streaming profile"
            }
        }
    }
}
