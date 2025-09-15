use crate::config::EncodingProfile;
use crate::encoding::FilterChain;
use crate::stream::preservation::StreamMapping;
use crate::utils::ffmpeg::VideoMetadata;
use crate::utils::{Error, FfmpegWrapper, Result};
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
    ) -> Result<tokio::process::Child> {
        let input_path_str = input_path.as_ref().to_string_lossy();
        let output_path_str = output_path.as_ref().to_string_lossy();

        let mut mode_params = HashMap::new();
        mode_params.insert("crf".to_string(), adaptive_crf.to_string());

        let x265_params = profile.build_x265_params_string_with_external_metadata(
            Some(&mode_params),
            Some(metadata.is_hdr),
            metadata.color_space.as_ref(),
            metadata.transfer_function.as_ref(),
            metadata.color_primaries.as_ref(),
            metadata.master_display.as_ref(),
            metadata.max_cll.as_ref(),
            external_metadata_params,
        );

        let mut args = vec!["-i".to_string(), input_path_str.to_string()];

        // Add filter chain
        args.extend(filters.build_ffmpeg_args());

        // Add comprehensive stream mapping from stream preservation analysis
        args.extend(stream_mapping.mapping_args.clone());

        // Add video encoding settings
        args.extend(vec!["-c:v".to_string(), "libx265".to_string()]);

        // Add pixel format as separate parameter if specified in profile
        if let Some(pix_fmt) = profile.get_pixel_format() {
            args.extend(vec!["-pix_fmt".to_string(), pix_fmt]);
        }

        // Add x265 parameters
        args.extend(vec!["-x265-params".to_string(), x265_params]);

        // Add metadata and stream-specific settings from stream preservation
        let stream_preservation =
            crate::stream::preservation::StreamPreservation::new(ffmpeg.clone());
        args.extend(stream_preservation.get_metadata_args(stream_mapping, custom_title));

        // Add progress monitoring for real-time feedback
        let progress_file = format!("/tmp/ffmpeg_progress_{}.txt", std::process::id());
        args.extend(vec![
            "-progress".to_string(),
            progress_file,
            "-nostats".to_string(),
            "-stats_period".to_string(),
            "0.5".to_string(),
        ]);

        // Add container optimization
        args.extend(vec![
            "-movflags".to_string(),
            "+faststart".to_string(),
            output_path_str.to_string(),
        ]);

        tracing::info!(
            "Starting CRF encoding with CRF={} ({} streams)",
            adaptive_crf,
            stream_mapping.video_streams.len()
                + stream_mapping.audio_streams.len()
                + stream_mapping.subtitle_streams.len()
                + stream_mapping.data_streams.len()
        );

        // Log the raw ffmpeg command to the log file
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
    ) -> Result<tokio::process::Child> {
        let input_path_str = input_path.as_ref().to_string_lossy();
        let output_path_str = output_path.as_ref().to_string_lossy();
        let stats_file = format!("/tmp/ffmpeg2pass_{}", uuid::Uuid::new_v4());

        tracing::info!(
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
    ) -> Result<()> {
        let mut mode_params = HashMap::new();
        mode_params.insert("pass".to_string(), "1".to_string());
        mode_params.insert("bitrate".to_string(), adaptive_bitrate.to_string());
        mode_params.insert("stats".to_string(), stats_file.to_string());
        mode_params.insert("preset".to_string(), "medium".to_string());
        mode_params.insert("no-slow-firstpass".to_string(), "1".to_string());

        if is_cbr {
            let vbv_bufsize = adaptive_bitrate * 15 / 10;
            mode_params.insert("vbv-bufsize".to_string(), vbv_bufsize.to_string());
            mode_params.insert("vbv-maxrate".to_string(), adaptive_bitrate.to_string());
            mode_params.insert("nal-hrd".to_string(), "cbr".to_string());
        }

        let x265_params = profile.build_x265_params_string_with_external_metadata(
            Some(&mode_params),
            Some(metadata.is_hdr),
            metadata.color_space.as_ref(),
            metadata.transfer_function.as_ref(),
            metadata.color_primaries.as_ref(),
            metadata.master_display.as_ref(),
            metadata.max_cll.as_ref(),
            external_metadata_params,
        );

        let mut args = vec!["-i".to_string(), input_path.to_string()];

        args.extend(filters.build_ffmpeg_args());

        args.extend(vec!["-c:v".to_string(), "libx265".to_string()]);

        // Add pixel format as separate parameter if specified in profile
        if let Some(pix_fmt) = profile.get_pixel_format() {
            args.extend(vec!["-pix_fmt".to_string(), pix_fmt]);
        }

        args.extend(vec![
            "-x265-params".to_string(),
            x265_params,
            "-an".to_string(),
            "-sn".to_string(),
            "-f".to_string(),
            "null".to_string(),
            "/dev/null".to_string(),
        ]);

        tracing::info!("Running pass 1/2...");
        let mut child = ffmpeg.start_encoding(input_path, "/dev/null", args).await?;
        let status = child.wait().await?;

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
    ) -> Result<tokio::process::Child> {
        let mut mode_params = HashMap::new();
        mode_params.insert("pass".to_string(), "2".to_string());
        mode_params.insert("bitrate".to_string(), adaptive_bitrate.to_string());
        mode_params.insert("stats".to_string(), stats_file.to_string());

        if is_cbr {
            let vbv_bufsize = adaptive_bitrate * 15 / 10;
            mode_params.insert("vbv-bufsize".to_string(), vbv_bufsize.to_string());
            mode_params.insert("vbv-maxrate".to_string(), adaptive_bitrate.to_string());
            mode_params.insert("nal-hrd".to_string(), "cbr".to_string());
        }

        let x265_params = profile.build_x265_params_string_with_external_metadata(
            Some(&mode_params),
            Some(metadata.is_hdr),
            metadata.color_space.as_ref(),
            metadata.transfer_function.as_ref(),
            metadata.color_primaries.as_ref(),
            metadata.master_display.as_ref(),
            metadata.max_cll.as_ref(),
            external_metadata_params,
        );

        let mut args = vec!["-i".to_string(), input_path.to_string()];

        // Add filter chain
        args.extend(filters.build_ffmpeg_args());

        // Add comprehensive stream mapping from stream preservation analysis
        args.extend(stream_mapping.mapping_args.clone());

        // Add video encoding settings
        args.extend(vec!["-c:v".to_string(), "libx265".to_string()]);

        // Add pixel format as separate parameter if specified in profile
        if let Some(pix_fmt) = profile.get_pixel_format() {
            args.extend(vec!["-pix_fmt".to_string(), pix_fmt]);
        }

        // Add x265 parameters
        args.extend(vec!["-x265-params".to_string(), x265_params]);

        // Add metadata and stream-specific settings from stream preservation
        let stream_preservation =
            crate::stream::preservation::StreamPreservation::new(ffmpeg.clone());
        args.extend(stream_preservation.get_metadata_args(stream_mapping, custom_title));

        // Add progress monitoring for real-time feedback
        let progress_file = format!("/tmp/ffmpeg_progress_{}.txt", std::process::id());
        args.extend(vec![
            "-progress".to_string(),
            progress_file,
            "-nostats".to_string(),
            "-stats_period".to_string(),
            "0.5".to_string(),
        ]);

        // Add container optimization
        args.extend(vec![
            "-movflags".to_string(),
            "+faststart".to_string(),
            output_path.to_string(),
        ]);

        tracing::info!("Running pass 2/2...");

        // Log the raw ffmpeg command to the log file
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
    ) -> Result<tokio::process::Child> {
        tracing::info!(
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
            )
            .await
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
