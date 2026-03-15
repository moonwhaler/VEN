use crate::config::EncodingProfile;
use crate::encoding::FilterChain;
use crate::stream::preservation::StreamMapping;
use crate::utils::ffmpeg::VideoMetadata;
use crate::utils::{Error, FfmpegWrapper, Result};
use indicatif::{ProgressBar, ProgressStyle};
use std::collections::HashMap;
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EncodingMode {
    CRF,
    ABR,
    CBR,
}

impl EncodingMode {
    pub fn from_string(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "crf" => Some(Self::CRF),
            "abr" => Some(Self::ABR),
            "cbr" => Some(Self::CBR),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::CRF => "crf",
            Self::ABR => "abr",
            Self::CBR => "cbr",
        }
    }
}

pub trait Encoder {
    #[allow(async_fn_in_trait)]
    #[allow(clippy::too_many_arguments)]
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
        file_logger: Option<&crate::utils::logging::FileLogger>,
        external_metadata_params: Option<&[(String, String)]>,
        hdr_passthrough_mode: bool,
        strip_dolby_vision: bool,
        strip_hdr10_plus: bool,
    ) -> Result<tokio::process::Child>;
}

pub struct CrfEncoder;

impl Encoder for CrfEncoder {
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
        _adaptive_bitrate: u32,
        custom_title: Option<&str>,
        file_logger: Option<&crate::utils::logging::FileLogger>,
        external_metadata_params: Option<&[(String, String)]>,
        hdr_passthrough_mode: bool,
        strip_dolby_vision: bool,
        strip_hdr10_plus: bool,
    ) -> Result<tokio::process::Child> {
        let input_path_str = input_path.as_ref().to_string_lossy();
        let output_path_str = output_path.as_ref().to_string_lossy();

        let mut mode_params = HashMap::new();
        mode_params.insert("crf".to_string(), adaptive_crf.to_string());

        // Handle Dolby Vision stripping
        if strip_dolby_vision {
            mode_params.insert("dolbyvision".to_string(), "0".to_string());
            tracing::info!("Stripping Dolby Vision metadata from output");
        }

        let x265_params = profile.build_x265_params_string_with_external_metadata_passthrough(
            Some(&mode_params),
            Some(metadata.is_hdr),
            metadata.color_space.as_ref(),
            metadata.transfer_function.as_ref(),
            metadata.color_primaries.as_ref(),
            metadata.master_display.as_ref(),
            metadata.max_cll.as_ref(),
            external_metadata_params,
            hdr_passthrough_mode,
        );

        let mut args = vec!["-i".to_string(), input_path_str.to_string()];

        // Add FFmpeg flags for metadata stripping
        if strip_dolby_vision {
            // Strip Dolby Vision RPU side data
            args.extend(vec!["-strict".to_string(), "-2".to_string()]);
        }

        if strip_hdr10_plus {
            // Strip HDR10+ dynamic metadata by not copying side data
            tracing::info!("Stripping HDR10+ metadata from output");
        }

        args.extend(vec![
            "-max_muxing_queue_size".to_string(),
            "1024".to_string(),
        ]);

        let filter_args = filters.build_ffmpeg_args();
        let uses_filter_complex = filter_args.contains(&"-filter_complex".to_string());
        args.extend(filter_args);

        let mut mapping_args = stream_mapping.mapping_args.clone();

        if uses_filter_complex {
            for i in 0..mapping_args.len() - 1 {
                if mapping_args[i] == "-map" && mapping_args[i + 1] == "0:v:0" {
                    mapping_args[i + 1] = "[v]".to_string();
                    break;
                }
            }
        }

        args.extend(mapping_args);

        args.extend(vec!["-c:v".to_string(), "libx265".to_string()]);

        if let Some(preset) = profile.get_preset() {
            args.extend(vec!["-preset".to_string(), preset]);
        }

        if let Some(profile_name) = profile.get_profile() {
            args.extend(vec!["-profile:v".to_string(), profile_name]);
        }

        if let Some(pix_fmt) = profile.get_pixel_format() {
            args.extend(vec!["-pix_fmt".to_string(), pix_fmt]);
        }

        args.extend(vec!["-x265-params".to_string(), x265_params]);

        args.extend(vec![
            "-default_mode".to_string(),
            "infer_no_subs".to_string(),
        ]);

        let stream_preservation =
            crate::stream::preservation::StreamPreservation::new(ffmpeg.clone());
        args.extend(stream_preservation.get_metadata_args(stream_mapping, custom_title));

        let progress_file = format!("/tmp/ffmpeg_progress_{}.txt", std::process::id());
        args.extend(vec![
            "-progress".to_string(),
            progress_file,
            "-nostats".to_string(),
            "-stats_period".to_string(),
            "1.0".to_string(),
        ]);

        args.extend(vec![
            "-movflags".to_string(),
            "+faststart".to_string(),
            output_path_str.to_string(),
        ]);

        tracing::debug!(
            "Starting CRF encoding with CRF={} ({} streams)",
            adaptive_crf,
            stream_mapping.video_streams.len()
                + stream_mapping.audio_streams.len()
                + stream_mapping.subtitle_streams.len()
                + stream_mapping.data_streams.len()
        );

        if let Some(logger) = file_logger {
            if let Err(e) = logger.log_ffmpeg_command(ffmpeg.get_ffmpeg_path(), &args) {
                tracing::warn!("Failed to log ffmpeg command: {}", e);
            }
        }

        ffmpeg.start_encoding(input_path, output_path, args).await
    }
}

pub struct AbrEncoder;

impl Encoder for AbrEncoder {
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
        file_logger: Option<&crate::utils::logging::FileLogger>,
        external_metadata_params: Option<&[(String, String)]>,
        hdr_passthrough_mode: bool,
        strip_dolby_vision: bool,
        strip_hdr10_plus: bool,
    ) -> Result<tokio::process::Child> {
        self.run_two_pass_encoding(
            ffmpeg,
            input_path,
            output_path,
            profile,
            filters,
            stream_mapping,
            metadata,
            adaptive_crf,
            adaptive_bitrate,
            custom_title,
            file_logger,
            external_metadata_params,
            false,
            hdr_passthrough_mode,
            strip_dolby_vision,
            strip_hdr10_plus,
        )
        .await
    }
}

impl AbrEncoder {
    #[allow(clippy::too_many_arguments)]
    async fn run_two_pass_encoding<P: AsRef<Path>>(
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
        file_logger: Option<&crate::utils::logging::FileLogger>,
        external_metadata_params: Option<&[(String, String)]>,
        is_cbr: bool,
        hdr_passthrough_mode: bool,
        strip_dolby_vision: bool,
        strip_hdr10_plus: bool,
    ) -> Result<tokio::process::Child> {
        let input_path_str = input_path.as_ref().to_string_lossy();
        let output_path_str = output_path.as_ref().to_string_lossy();
        let stats_file = format!("/tmp/ffmpeg2pass_{}", uuid::Uuid::new_v4());

        tracing::debug!(
            "Starting two-pass {} encoding (bitrate={}kbps)",
            if is_cbr { "CBR" } else { "ABR" },
            adaptive_bitrate
        );

        let pass1_result = self
            .run_first_pass(
                ffmpeg,
                &input_path_str,
                profile,
                filters,
                metadata,
                adaptive_bitrate,
                external_metadata_params,
                &stats_file,
                is_cbr,
                hdr_passthrough_mode,
                strip_dolby_vision,
                strip_hdr10_plus,
            )
            .await;

        if let Err(e) = pass1_result {
            self.cleanup_stats_files(&stats_file);
            return Err(e);
        }

        let pass2_result = self
            .run_second_pass(
                ffmpeg,
                &input_path_str,
                &output_path_str,
                profile,
                filters,
                stream_mapping,
                metadata,
                adaptive_crf,
                adaptive_bitrate,
                custom_title,
                file_logger,
                external_metadata_params,
                &stats_file,
                is_cbr,
                hdr_passthrough_mode,
                strip_dolby_vision,
                strip_hdr10_plus,
            )
            .await;

        self.cleanup_stats_files(&stats_file);
        pass2_result
    }

    #[allow(clippy::too_many_arguments)]
    async fn run_first_pass(
        &self,
        ffmpeg: &FfmpegWrapper,
        input_path: &str,
        profile: &EncodingProfile,
        filters: &FilterChain,
        metadata: &VideoMetadata,
        adaptive_bitrate: u32,
        external_metadata_params: Option<&[(String, String)]>,
        stats_file: &str,
        is_cbr: bool,
        hdr_passthrough_mode: bool,
        strip_dolby_vision: bool,
        _strip_hdr10_plus: bool,
    ) -> Result<()> {
        let mut mode_params = HashMap::new();
        mode_params.insert("pass".to_string(), "1".to_string());
        mode_params.insert("bitrate".to_string(), adaptive_bitrate.to_string());
        mode_params.insert("stats".to_string(), stats_file.to_string());
        if let Some(preset) = profile.get_preset() {
            mode_params.insert("preset".to_string(), preset);
        }
        mode_params.insert("no-slow-firstpass".to_string(), "1".to_string());

        // Handle Dolby Vision stripping
        if strip_dolby_vision {
            mode_params.insert("dolbyvision".to_string(), "0".to_string());
        }

        if is_cbr {
            if !profile.x265_params.contains_key("vbv-bufsize")
                && !profile.x265_params.contains_key("vbv-maxrate")
            {
                let vbv_bufsize = adaptive_bitrate * 15 / 10;
                mode_params.insert("vbv-bufsize".to_string(), vbv_bufsize.to_string());
                mode_params.insert("vbv-maxrate".to_string(), adaptive_bitrate.to_string());
            }
            mode_params.insert("nal-hrd".to_string(), "cbr".to_string());
        }

        let x265_params = profile.build_x265_params_string_with_external_metadata_passthrough(
            Some(&mode_params),
            Some(metadata.is_hdr),
            metadata.color_space.as_ref(),
            metadata.transfer_function.as_ref(),
            metadata.color_primaries.as_ref(),
            metadata.master_display.as_ref(),
            metadata.max_cll.as_ref(),
            external_metadata_params,
            hdr_passthrough_mode,
        );

        let mut args = vec!["-i".to_string(), input_path.to_string()];

        // Add FFmpeg flags for metadata stripping in pass 1
        if strip_dolby_vision {
            args.extend(vec!["-strict".to_string(), "-2".to_string()]);
        }

        args.extend(vec![
            "-max_muxing_queue_size".to_string(),
            "1024".to_string(),
        ]);

        let filter_args = filters.build_ffmpeg_args();
        let uses_filter_complex = filter_args.contains(&"-filter_complex".to_string());
        args.extend(filter_args);

        // For filter_complex, we need to map the output
        if uses_filter_complex {
            args.extend(vec!["-map".to_string(), "[v]".to_string()]);
        } else {
            args.extend(vec!["-map".to_string(), "0:v".to_string()]);
        }

        args.extend(vec!["-c:v".to_string(), "libx265".to_string()]);

        if let Some(pix_fmt) = profile.get_pixel_format() {
            args.extend(vec!["-pix_fmt".to_string(), pix_fmt]);
        }

        let progress_file = format!("/tmp/ffmpeg_progress_{}.txt", std::process::id());
        args.extend(vec![
            "-x265-params".to_string(),
            x265_params,
            "-an".to_string(),
            "-sn".to_string(),
            "-progress".to_string(),
            progress_file.clone(),
            "-nostats".to_string(),
            "-stats_period".to_string(),
            "1.0".to_string(),
            "-f".to_string(),
            "null".to_string(),
            "/dev/null".to_string(),
        ]);

        tracing::debug!("Running pass 1/2...");
        let mut child = ffmpeg.start_encoding(input_path, "/dev/null", args).await?;

        // Monitor pass 1 progress instead of blocking silently
        let total_duration = metadata.duration;
        let pb = ProgressBar::new(10000);
        pb.set_style(
            ProgressStyle::with_template(
                "[{wide_bar:.cyan/blue}] {percent_precise:>5}% (Pass 1/2 - analysis) | {msg}",
            )
            .unwrap()
            .with_key(
                "percent_precise",
                |state: &indicatif::ProgressState, w: &mut dyn std::fmt::Write| {
                    _ = write!(w, "{:>4.1}", state.fraction() * 100.0);
                },
            )
            .progress_chars("█▉▊▋▌▍▎▏ "),
        );

        let start_time = std::time::Instant::now();
        let mut interval_timer = tokio::time::interval(std::time::Duration::from_millis(1000));

        let status = loop {
            interval_timer.tick().await;

            match child.try_wait()? {
                Some(status) => {
                    pb.set_position(10000);
                    let duration = start_time.elapsed();
                    pb.finish_with_message(format!(
                        "Pass 1 completed in {}",
                        format_pass_duration(duration)
                    ));
                    let _ = std::fs::remove_file(&progress_file);
                    break status;
                }
                None => {
                    let progress_path = std::path::Path::new(&progress_file);
                    if progress_path.exists() {
                        if let Ok(content) = tokio::fs::read_to_string(&progress_file).await {
                            if let Some((time_secs, fps, speed)) =
                                parse_pass1_progress(&content)
                            {
                                if total_duration > 0.0 && time_secs > 0.0 {
                                    let progress =
                                        (time_secs / total_duration).min(1.0);
                                    pb.set_position((progress * 10000.0) as u64);

                                    let mut parts = vec![];
                                    if let Some(f) = fps {
                                        parts.push(format!("{:.1}fps", f));
                                    }
                                    if let Some(s) = speed {
                                        parts.push(format!("{:.1}x", s));
                                    }

                                    // ETA calculation
                                    if progress > 0.005 {
                                        let elapsed =
                                            start_time.elapsed().as_secs_f64();
                                        let eta =
                                            (elapsed / progress) - elapsed;
                                        if eta > 0.0 && eta < 24.0 * 3600.0 {
                                            let eta_dur = std::time::Duration::from_secs_f64(eta);
                                            parts.push(format!(
                                                "ETA {}",
                                                format_pass_duration(eta_dur)
                                            ));
                                        }
                                    }

                                    if !parts.is_empty() {
                                        pb.set_message(parts.join(" • "));
                                    }
                                }
                            }
                        }
                    }
                }
            }
        };

        if !status.success() {
            return Err(Error::encoding("First pass encoding failed"));
        }

        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    async fn run_second_pass(
        &self,
        ffmpeg: &FfmpegWrapper,
        input_path: &str,
        output_path: &str,
        profile: &EncodingProfile,
        filters: &FilterChain,
        stream_mapping: &StreamMapping,
        metadata: &VideoMetadata,
        _adaptive_crf: f32,
        adaptive_bitrate: u32,
        custom_title: Option<&str>,
        file_logger: Option<&crate::utils::logging::FileLogger>,
        external_metadata_params: Option<&[(String, String)]>,
        stats_file: &str,
        is_cbr: bool,
        hdr_passthrough_mode: bool,
        strip_dolby_vision: bool,
        _strip_hdr10_plus: bool,
    ) -> Result<tokio::process::Child> {
        let mut mode_params = HashMap::new();
        mode_params.insert("pass".to_string(), "2".to_string());
        mode_params.insert("bitrate".to_string(), adaptive_bitrate.to_string());
        mode_params.insert("stats".to_string(), stats_file.to_string());

        // Handle Dolby Vision stripping
        if strip_dolby_vision {
            mode_params.insert("dolbyvision".to_string(), "0".to_string());
        }

        if is_cbr {
            if !profile.x265_params.contains_key("vbv-bufsize")
                && !profile.x265_params.contains_key("vbv-maxrate")
            {
                let vbv_bufsize = adaptive_bitrate * 15 / 10;
                mode_params.insert("vbv-bufsize".to_string(), vbv_bufsize.to_string());
                mode_params.insert("vbv-maxrate".to_string(), adaptive_bitrate.to_string());
            }
            mode_params.insert("nal-hrd".to_string(), "cbr".to_string());
        }

        let x265_params = profile.build_x265_params_string_with_external_metadata_passthrough(
            Some(&mode_params),
            Some(metadata.is_hdr),
            metadata.color_space.as_ref(),
            metadata.transfer_function.as_ref(),
            metadata.color_primaries.as_ref(),
            metadata.master_display.as_ref(),
            metadata.max_cll.as_ref(),
            external_metadata_params,
            hdr_passthrough_mode,
        );

        let mut args = vec!["-i".to_string(), input_path.to_string()];

        // Add FFmpeg flags for metadata stripping in pass 2
        if strip_dolby_vision {
            args.extend(vec!["-strict".to_string(), "-2".to_string()]);
        }

        args.extend(vec![
            "-max_muxing_queue_size".to_string(),
            "1024".to_string(),
        ]);

        let filter_args = filters.build_ffmpeg_args();
        let uses_filter_complex = filter_args.contains(&"-filter_complex".to_string());
        args.extend(filter_args);

        let mut mapping_args = stream_mapping.mapping_args.clone();

        if uses_filter_complex {
            for i in 0..mapping_args.len() - 1 {
                if mapping_args[i] == "-map" && mapping_args[i + 1] == "0:v:0" {
                    mapping_args[i + 1] = "[v]".to_string();
                    break;
                }
            }
        }

        args.extend(mapping_args);

        args.extend(vec!["-c:v".to_string(), "libx265".to_string()]);

        if let Some(preset) = profile.get_preset() {
            args.extend(vec!["-preset".to_string(), preset]);
        }

        if let Some(profile_name) = profile.get_profile() {
            args.extend(vec!["-profile:v".to_string(), profile_name]);
        }

        if let Some(pix_fmt) = profile.get_pixel_format() {
            args.extend(vec!["-pix_fmt".to_string(), pix_fmt]);
        }

        args.extend(vec!["-x265-params".to_string(), x265_params]);

        args.extend(vec![
            "-default_mode".to_string(),
            "infer_no_subs".to_string(),
        ]);

        let stream_preservation =
            crate::stream::preservation::StreamPreservation::new(ffmpeg.clone());
        args.extend(stream_preservation.get_metadata_args(stream_mapping, custom_title));

        let progress_file = format!("/tmp/ffmpeg_progress_{}.txt", std::process::id());
        args.extend(vec![
            "-progress".to_string(),
            progress_file,
            "-nostats".to_string(),
            "-stats_period".to_string(),
            "1.0".to_string(),
        ]);

        args.extend(vec![
            "-movflags".to_string(),
            "+faststart".to_string(),
            output_path.to_string(),
        ]);

        tracing::debug!("Running pass 2/2...");

        if let Some(logger) = file_logger {
            if let Err(e) = logger.log_ffmpeg_command(ffmpeg.get_ffmpeg_path(), &args) {
                tracing::warn!("Failed to log ffmpeg command: {}", e);
            }
        }

        ffmpeg.start_encoding(input_path, output_path, args).await
    }

    fn cleanup_stats_files(&self, stats_prefix: &str) {
        let stats_files = [
            format!("{}-0.log", stats_prefix),
            format!("{}-0.log.mbtree", stats_prefix),
            format!("{}-0.log.temp", stats_prefix),
        ];

        for file in &stats_files {
            if std::path::Path::new(file).exists() {
                let _ = std::fs::remove_file(file);
            }
        }
    }
}

pub struct CbrEncoder {
    abr_encoder: AbrEncoder,
}

impl CbrEncoder {
    pub fn new() -> Self {
        Self {
            abr_encoder: AbrEncoder,
        }
    }
}

impl Default for CbrEncoder {
    fn default() -> Self {
        Self::new()
    }
}

impl Encoder for CbrEncoder {
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
        file_logger: Option<&crate::utils::logging::FileLogger>,
        external_metadata_params: Option<&[(String, String)]>,
        hdr_passthrough_mode: bool,
        strip_dolby_vision: bool,
        strip_hdr10_plus: bool,
    ) -> Result<tokio::process::Child> {
        tracing::debug!(
            "Starting CBR encoding (constant bitrate={}kbps)",
            adaptive_bitrate
        );

        self.abr_encoder
            .run_two_pass_encoding(
                ffmpeg,
                input_path,
                output_path,
                profile,
                filters,
                stream_mapping,
                metadata,
                adaptive_crf,
                adaptive_bitrate,
                custom_title,
                file_logger,
                external_metadata_params,
                true,
                hdr_passthrough_mode,
                strip_dolby_vision,
                strip_hdr10_plus,
            )
            .await
    }
}

/// Parse progress data from ffmpeg's `-progress` output file for pass 1 monitoring.
/// Returns (time_in_seconds, optional_fps, optional_speed).
fn parse_pass1_progress(content: &str) -> Option<(f64, Option<f32>, Option<f32>)> {
    let mut time_secs = 0.0f64;
    let mut fps: Option<f32> = None;
    let mut speed: Option<f32> = None;

    // Read the last ~20 lines to get the most recent progress block
    let lines: Vec<&str> = content.lines().collect();
    let last_lines: Vec<&str> = lines.iter().rev().take(20).cloned().collect();

    for line in last_lines.iter().rev() {
        let line = line.trim();
        if let Some((key, value)) = line.split_once('=') {
            let key = key.trim();
            let value = value.trim();
            match key {
                "out_time_us" => {
                    if value != "N/A" {
                        if let Ok(us) = value.parse::<u64>() {
                            time_secs = us as f64 / 1_000_000.0;
                        }
                    }
                }
                "fps" => {
                    if value != "N/A" {
                        fps = value.parse().ok();
                    }
                }
                "speed" => {
                    if value != "N/A" {
                        speed = value.trim_end_matches('x').parse().ok();
                    }
                }
                _ => {}
            }
        }
    }

    if time_secs > 0.0 {
        Some((time_secs, fps, speed))
    } else {
        None
    }
}

fn format_pass_duration(duration: std::time::Duration) -> String {
    let total_secs = duration.as_secs();
    let hours = total_secs / 3600;
    let minutes = (total_secs % 3600) / 60;
    let seconds = total_secs % 60;

    if hours > 0 {
        format!("{}:{:02}:{:02}", hours, minutes, seconds)
    } else {
        format!("{}:{:02}", minutes, seconds)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encoding_mode_from_str() {
        assert_eq!(EncodingMode::from_string("crf"), Some(EncodingMode::CRF));
        assert_eq!(EncodingMode::from_string("CRF"), Some(EncodingMode::CRF));
        assert_eq!(EncodingMode::from_string("abr"), Some(EncodingMode::ABR));
        assert_eq!(EncodingMode::from_string("cbr"), Some(EncodingMode::CBR));
        assert_eq!(EncodingMode::from_string("invalid"), None);
    }

    #[test]
    fn test_encoding_mode_as_str() {
        assert_eq!(EncodingMode::CRF.as_str(), "crf");
        assert_eq!(EncodingMode::ABR.as_str(), "abr");
        assert_eq!(EncodingMode::CBR.as_str(), "cbr");
    }
}
