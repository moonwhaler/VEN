use crate::{
    analysis::ContentAnalyzer,
    cli::CliArgs,
    config::{Config, EncodingProfile, ProfileManager, StreamSelectionProfileManager},
    encoding::{
        modes::Encoder, AbrEncoder, CbrEncoder, CrfEncoder, EncodingMode, FilterBuilder,
        FilterChain,
    },
    metadata_workflow::MetadataWorkflowManager,
    progress::ProgressMonitor,
    stream::preservation::StreamPreservation,
    utils::{ffmpeg::VideoMetadata, Error, FfmpegWrapper, FileLogger, Result},
    ContentEncodingApproach, UnifiedContentManager,
};
use std::path::Path;
use tracing::info;

pub struct VideoProcessor<'a> {
    ffmpeg: &'a FfmpegWrapper,
    stream_preservation: &'a StreamPreservation,
    args: &'a CliArgs,
    config: &'a Config,
    profile_manager: &'a mut ProfileManager,
    stream_profile_manager: StreamSelectionProfileManager,
    input_path: &'a Path,
    output_path: &'a Path,
}

impl<'a> VideoProcessor<'a> {
    pub fn new(
        ffmpeg: &'a FfmpegWrapper,
        stream_preservation: &'a StreamPreservation,
        args: &'a CliArgs,
        config: &'a Config,
        profile_manager: &'a mut ProfileManager,
        input_path: &'a Path,
        output_path: &'a Path,
    ) -> Result<Self> {
        let stream_profile_manager =
            StreamSelectionProfileManager::new(config.stream_selection_profiles.clone())?;

        Ok(Self {
            ffmpeg,
            stream_preservation,
            args,
            config,
            profile_manager,
            stream_profile_manager,
            input_path,
            output_path,
        })
    }

    pub async fn run(&mut self) -> Result<()> {
        let metadata = self.get_metadata().await?;

        let content_manager = UnifiedContentManager::new(
            self.config.analysis.hdr.clone().unwrap_or_default(),
            self.config.analysis.dolby_vision.clone(),
            self.config.tools.hdr10plus_tool.clone(),
        );
        let hdr_analysis = content_manager
            .analyze_hdr_only(self.ffmpeg, self.input_path)
            .await?;

        let is_advanced_content = hdr_analysis.metadata.format != crate::hdr::HdrFormat::None;
        let (crop_values, crop_sample_timestamps, crop_analysis_result) =
            self.detect_crop(is_advanced_content, &metadata).await?;

        let content_analysis = content_manager
            .analyze_content_with_hdr_reuse(self.ffmpeg, self.input_path, Some(hdr_analysis))
            .await?;
        let metadata_workflow = self.initialize_metadata_workflow().await?;
        let extracted_metadata = metadata_workflow
            .extract_metadata(
                self.input_path,
                &content_analysis.recommended_approach,
                &content_analysis.dolby_vision,
                &content_analysis.hdr_analysis,
            )
            .await?;

        self.log_content_analysis(&metadata, &content_analysis);

        let selected_profile = self.select_profile(&metadata).await?;
        let file_logger = FileLogger::new(self.output_path)?;

        let adaptive_crf =
            selected_profile.base_crf + content_analysis.encoding_adjustments.crf_adjustment;
        let adaptive_bitrate = ((selected_profile.bitrate as f32)
            * content_analysis.encoding_adjustments.bitrate_multiplier)
            as u32;

        self.log_parameter_adjustments(
            &content_analysis,
            &selected_profile,
            adaptive_crf,
            adaptive_bitrate,
        );

        let is_advanced_content = !matches!(
            content_analysis.recommended_approach,
            ContentEncodingApproach::SDR
        );
        let x265_params_preview =
            self.build_x265_params_preview(&selected_profile, &metadata, is_advanced_content);
        self.log_x265_params(&content_analysis, &x265_params_preview, is_advanced_content);

        let filter_chain = self.build_filter_chain(crop_values.as_deref())?;
        let encoding_mode = self.get_encoding_mode()?;
        let stream_mapping = self.analyze_streams().await?;

        self.log_initial_settings(
            &file_logger,
            &selected_profile,
            adaptive_crf,
            adaptive_bitrate,
            &filter_chain,
            &stream_mapping,
            &metadata,
            &content_analysis,
            &x265_params_preview,
            crop_values.as_deref(),
            &crop_sample_timestamps,
            crop_analysis_result.as_ref(),
            is_advanced_content,
        )?;

        let needs_post_processing = metadata_workflow.needs_post_processing(&extracted_metadata);
        let actual_output_path = if needs_post_processing {
            metadata_workflow.get_temp_output_path(self.output_path)
        } else {
            self.output_path.to_path_buf()
        };

        let external_metadata_params =
            metadata_workflow.build_external_metadata_params(&extracted_metadata);
        let external_params_ref = if external_metadata_params.is_empty() {
            None
        } else {
            Some(external_metadata_params.as_slice())
        };

        // Start timer for encoding duration
        let encoding_start = std::time::Instant::now();

        let child = self
            .start_encoding(
                &actual_output_path,
                &selected_profile,
                &filter_chain,
                &stream_mapping,
                &metadata,
                adaptive_crf,
                adaptive_bitrate,
                encoding_mode,
                &file_logger,
                external_params_ref,
            )
            .await?;

        let mut progress_monitor = self.create_progress_monitor(&metadata, encoding_mode);
        let status = progress_monitor.monitor_encoding(child).await?;

        if status.success() && needs_post_processing {
            match metadata_workflow
                .inject_metadata(
                    &actual_output_path,
                    &self.output_path.to_path_buf(),
                    &extracted_metadata,
                    metadata.fps,
                )
                .await
            {
                Ok(_) => {}
                Err(e) => {
                    if actual_output_path.exists() {
                        let _ = tokio::fs::remove_file(&actual_output_path).await;
                        tracing::debug!(
                            "Cleaned up temporary file after metadata injection failure: {}",
                            actual_output_path.display()
                        );
                    }
                    return Err(e);
                }
            }
        } else if needs_post_processing && !status.success() && actual_output_path.exists() {
            if let Err(e) = tokio::fs::remove_file(&actual_output_path).await {
                tracing::warn!(
                    "Failed to clean up temporary file after encoding failure: {}",
                    e
                );
            } else {
                tracing::debug!(
                    "Cleaned up temporary file after encoding failure: {}",
                    actual_output_path.display()
                );
            }
        }

        let encoding_duration = encoding_start.elapsed();
        self.finalize_logging(&file_logger, status, encoding_duration)?;

        metadata_workflow.cleanup().await?;
        extracted_metadata.cleanup();

        Ok(())
    }

    async fn get_metadata(&self) -> Result<VideoMetadata> {
        info!("Getting video metadata for: {}", self.input_path.display());
        self.ffmpeg.get_video_metadata(self.input_path).await
    }

    async fn initialize_metadata_workflow(&self) -> Result<MetadataWorkflowManager> {
        info!("Initializing metadata workflow manager...");
        MetadataWorkflowManager::new(self.config).await
    }

    fn log_content_analysis(
        &self,
        metadata: &VideoMetadata,
        content_analysis: &crate::ContentAnalysisResult,
    ) {
        match &content_analysis.recommended_approach {
            ContentEncodingApproach::SDR => {
                info!("SDR CONTENT DETECTED");
            }
            ContentEncodingApproach::HDR(hdr_result) => {
                info!("HDR CONTENT DETECTED");
                info!("  Format: {:?}", hdr_result.metadata.format);
                if let Some(ref color_space) = metadata.color_space {
                    info!("  Color Space: {}", color_space);
                }
            }
            ContentEncodingApproach::DolbyVision(dv_info) => {
                info!("DOLBY VISION CONTENT DETECTED");
                info!("  Profile: {}", dv_info.profile.as_str());
            }
            ContentEncodingApproach::DolbyVisionWithHDR10Plus(dv_info, _) => {
                info!("DUAL FORMAT CONTENT DETECTED: DOLBY VISION + HDR10+");
                info!("  Dolby Vision Profile: {}", dv_info.profile.as_str());
            }
        }
    }

    async fn select_profile(&self, metadata: &VideoMetadata) -> Result<EncodingProfile> {
        if self.args.profile == "auto" {
            info!("Auto-selecting profile based on content analysis...");

            let content_analyzer = ContentAnalyzer::new();
            let classification = content_analyzer.classify_content(metadata).await?;
            let content_type = classification.content_type;

            if let Some(profile) = self.profile_manager.recommend_profile_for_resolution(
                metadata.width,
                metadata.height,
                content_type,
            ) {
                info!(
                    "Selected profile based on content analysis: {} (confidence: {:.1}%)",
                    profile.name,
                    classification.confidence * 100.0
                );
                Ok(profile.clone())
            } else {
                info!("No specific profile found for content type, using default 'movie' profile");
                self.profile_manager
                    .get_profile("movie")
                    .cloned()
                    .ok_or_else(|| Error::profile("Default 'movie' profile not found"))
            }
        } else {
            self.profile_manager
                .get_profile(&self.args.profile)
                .ok_or_else(|| Error::profile(format!("Profile '{}' not found", self.args.profile)))
                .cloned()
        }
    }

    fn log_parameter_adjustments(
        &self,
        content_analysis: &crate::ContentAnalysisResult,
        selected_profile: &EncodingProfile,
        adaptive_crf: f32,
        adaptive_bitrate: u32,
    ) {
        match &content_analysis.recommended_approach {
            ContentEncodingApproach::SDR => {
                info!(
                    "Using standard encoding parameters (SDR): CRF={:.1}, Bitrate={}kbps",
                    adaptive_crf, adaptive_bitrate
                );
            }
            _ => {
                info!("PARAMETER ADJUSTMENTS:");
                info!(
                    "  Base CRF: {} -> Adjusted CRF: {:.1} (+{:.1})",
                    selected_profile.base_crf,
                    adaptive_crf,
                    content_analysis.encoding_adjustments.crf_adjustment
                );
                info!(
                    "  Base Bitrate: {} -> Adjusted Bitrate: {} ({:.1}x multiplier)",
                    selected_profile.bitrate,
                    adaptive_bitrate,
                    content_analysis.encoding_adjustments.bitrate_multiplier
                );
            }
        }
    }

    fn build_x265_params_preview(
        &self,
        selected_profile: &EncodingProfile,
        metadata: &VideoMetadata,
        is_advanced_content: bool,
    ) -> String {
        selected_profile.build_x265_params_string_with_hdr_passthrough(
            None,
            Some(is_advanced_content),
            metadata.color_space.as_ref(),
            metadata.transfer_function.as_ref(),
            metadata.color_primaries.as_ref(),
            metadata.master_display.as_ref(),
            metadata.max_cll.as_ref(),
            false, // Default to non-passthrough mode
        )
    }

    fn log_x265_params(
        &self,
        content_analysis: &crate::ContentAnalysisResult,
        x265_params_preview: &str,
        is_advanced_content: bool,
    ) {
        if is_advanced_content {
            match &content_analysis.recommended_approach {
                ContentEncodingApproach::HDR(_) => info!("HDR x265 parameters injected:"),
                ContentEncodingApproach::DolbyVision(_) => {
                    info!("Dolby Vision x265 parameters injected:")
                }
                ContentEncodingApproach::DolbyVisionWithHDR10Plus(_, _) => {
                    info!("Dual format (DV+HDR10+) x265 parameters injected:")
                }
                _ => {}
            }
            let params: Vec<&str> = x265_params_preview.split(':').collect();
            let special_params: Vec<&str> = params
                .iter()
                .filter(|p| {
                    p.contains("colormatrix")
                        || p.contains("transfer")
                        || p.contains("colorprim")
                        || p.contains("master-display")
                        || p.contains("max-cll")
                })
                .copied()
                .collect();
            if !special_params.is_empty() {
                for param in special_params {
                    info!("  -> {}", param);
                }
            }
        }
    }

    async fn detect_crop(
        &self,
        is_advanced_content: bool,
        metadata: &VideoMetadata,
    ) -> Result<(
        Option<String>,
        Vec<f64>,
        Option<crate::analysis::CropAnalysisResult>,
    )> {
        if self.config.analysis.crop_detection.enabled {
            use crate::analysis::CropDetector;
            let crop_detector = CropDetector::new(self.config.analysis.crop_detection.clone());
            let crop_analysis = crop_detector
                .detect_crop_values(
                    self.input_path,
                    metadata.duration,
                    metadata.width,
                    metadata.height,
                    is_advanced_content,
                )
                .await?;
            let sample_timestamps = self
                .config
                .analysis
                .crop_detection
                .get_sample_timestamps(metadata.duration);
            let crop_values = crop_analysis
                .crop_values
                .as_ref()
                .map(|cv| cv.to_ffmpeg_string());
            Ok((crop_values, sample_timestamps, Some(crop_analysis)))
        } else {
            Ok((None, vec![], None))
        }
    }

    fn build_filter_chain(&self, crop_values: Option<&str>) -> Result<FilterChain> {
        Ok(FilterBuilder::new(self.config)
            .with_deinterlace(self.args.deinterlace)?
            .with_denoise(self.args.denoise)
            .with_crop(crop_values)?
            .build())
    }

    fn get_encoding_mode(&self) -> Result<EncodingMode> {
        EncodingMode::from_string(&self.args.mode)
            .ok_or_else(|| Error::encoding(format!("Invalid encoding mode: {}", self.args.mode)))
    }

    async fn analyze_streams(&self) -> Result<crate::stream::preservation::StreamMapping> {
        if let Some(profile_name) = &self.args.stream_selection_profile {
            let profile = self.stream_profile_manager.get_profile(profile_name)?;
            self.stream_preservation
                .analyze_streams_with_profile(self.input_path, profile)
                .await
        } else {
            self.stream_preservation
                .analyze_streams(self.input_path)
                .await
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn log_initial_settings(
        &self,
        file_logger: &FileLogger,
        selected_profile: &EncodingProfile,
        adaptive_crf: f32,
        adaptive_bitrate: u32,
        filter_chain: &FilterChain,
        stream_mapping: &crate::stream::preservation::StreamMapping,
        metadata: &VideoMetadata,
        content_analysis: &crate::ContentAnalysisResult,
        x265_params_preview: &str,
        crop_values: Option<&str>,
        crop_sample_timestamps: &[f64],
        crop_analysis_result: Option<&crate::analysis::CropAnalysisResult>,
        is_advanced_content: bool,
    ) -> Result<()> {
        file_logger.log_encoding_settings(
            self.input_path,
            self.output_path,
            &self.args.profile,
            selected_profile,
            &self.args.mode,
            adaptive_crf,
            adaptive_bitrate,
            Some(&filter_chain.to_string()),
            &format!("{:?}", stream_mapping),
        )?;
        file_logger.log_analysis_results(metadata, None, Some(content_analysis))?;
        file_logger.log_encoding_progress(&format!("x265 parameters: {}", x265_params_preview))?;
        let detection_method = if let Some(analysis) = crop_analysis_result {
            &analysis.detection_method
        } else if self.config.analysis.crop_detection.enabled {
            "automatic_detection"
        } else {
            "disabled"
        };
        file_logger.log_crop_detection_results(
            self.config.analysis.crop_detection.enabled,
            self.config.analysis.crop_detection.sample_count,
            crop_sample_timestamps,
            crop_values,
            detection_method,
            self.config.analysis.crop_detection.sdr_crop_limit,
            self.config.analysis.crop_detection.hdr_crop_limit,
            is_advanced_content,
        )?;
        if let Some(analysis) = crop_analysis_result {
            file_logger.log_encoding_progress(&format!(
                "Crop Analysis: {:.1}% confidence, {:.1}% pixel change, {} samples processed",
                analysis.confidence,
                analysis.pixel_change_percent,
                analysis.sample_results.len()
            ))?;
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    async fn start_encoding(
        &self,
        actual_output_path: &Path,
        selected_profile: &EncodingProfile,
        filter_chain: &FilterChain,
        stream_mapping: &crate::stream::preservation::StreamMapping,
        metadata: &VideoMetadata,
        adaptive_crf: f32,
        adaptive_bitrate: u32,
        encoding_mode: EncodingMode,
        file_logger: &FileLogger,
        external_params_ref: Option<&[(String, String)]>,
    ) -> Result<tokio::process::Child> {
        match encoding_mode {
            EncodingMode::CRF => {
                CrfEncoder
                    .encode(
                        self.ffmpeg,
                        self.input_path,
                        actual_output_path,
                        selected_profile,
                        filter_chain,
                        stream_mapping,
                        metadata,
                        adaptive_crf,
                        adaptive_bitrate,
                        self.args.title.as_deref(),
                        Some(file_logger),
                        external_params_ref,
                        false, // Default to non-passthrough mode
                    )
                    .await
            }
            EncodingMode::ABR => {
                AbrEncoder
                    .encode(
                        self.ffmpeg,
                        self.input_path,
                        actual_output_path,
                        selected_profile,
                        filter_chain,
                        stream_mapping,
                        metadata,
                        adaptive_crf,
                        adaptive_bitrate,
                        self.args.title.as_deref(),
                        Some(file_logger),
                        external_params_ref,
                        false,
                    )
                    .await
            }
            EncodingMode::CBR => {
                CbrEncoder::new()
                    .encode(
                        self.ffmpeg,
                        self.input_path,
                        actual_output_path,
                        selected_profile,
                        filter_chain,
                        stream_mapping,
                        metadata,
                        adaptive_crf,
                        adaptive_bitrate,
                        self.args.title.as_deref(),
                        Some(file_logger),
                        external_params_ref,
                        false,
                    )
                    .await
            }
        }
    }

    fn create_progress_monitor(
        &self,
        metadata: &VideoMetadata,
        encoding_mode: EncodingMode,
    ) -> ProgressMonitor {
        let source_file_size = std::fs::metadata(self.input_path).map(|m| m.len()).ok();

        let progress_monitor = ProgressMonitor::new(
            metadata.duration,
            metadata.fps,
            self.ffmpeg.clone(),
            encoding_mode,
            source_file_size,
        );
        let total_frames = if metadata.fps > 0.0 && metadata.duration > 0.0 {
            (metadata.duration * metadata.fps as f64) as u32
        } else {
            0
        };
        progress_monitor.set_message(&format!(
            "Encoding {} ({}x{}, {:.1}fps, {} frames)",
            self.input_path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy(),
            metadata.width,
            metadata.height,
            metadata.fps,
            total_frames
        ));
        progress_monitor
    }

    fn finalize_logging(
        &self,
        file_logger: &FileLogger,
        status: std::process::ExitStatus,
        duration: std::time::Duration,
    ) -> Result<()> {
        let output_size = std::fs::metadata(self.output_path).map(|m| m.len()).ok();
        let exit_code = status.code();
        if status.success() {
            if let Some(size) = output_size {
                info!(
                    "Encoding completed successfully in {:.2}s, output size: {:.2} MB",
                    duration.as_secs_f64(),
                    size as f64 / 1_048_576.0
                );
            } else {
                info!(
                    "Encoding completed successfully in {:.2}s",
                    duration.as_secs_f64()
                );
            }
            file_logger.log_encoding_complete(true, duration, output_size, exit_code)?;
            info!(
                "Encoding log saved to: {}",
                file_logger.get_log_path().display()
            );
        } else {
            file_logger.log_encoding_complete(false, duration, output_size, exit_code)?;
            return Err(Error::encoding(format!(
                "Encoding failed with exit code: {}",
                exit_code.unwrap_or(-1)
            )));
        }
        Ok(())
    }
}
