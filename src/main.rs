use clap::Parser;
use tracing::info;

use ffmpeg_autoencoder::{
    cli::{handle_commands, CliArgs},
    config::{Config, ProfileManager},
    encoding::{modes::Encoder, AbrEncoder, CbrEncoder, CrfEncoder, EncodingMode, FilterBuilder},
    metadata_workflow::MetadataWorkflowManager,
    stream::preservation::StreamPreservation,
    progress::ProgressMonitor,
    utils::{
        find_video_files, generate_uuid_filename, setup_logging, Error, FfmpegWrapper, FileLogger,
        Result,
    },
    ContentEncodingApproach, UnifiedContentManager,
};

#[tokio::main]
async fn main() -> Result<()> {
    let args = CliArgs::parse();

    // If no arguments provided and no info commands, show help
    if !args.is_info_command() && args.input.is_empty() {
        use clap::CommandFactory;
        let mut cmd = CliArgs::command();
        cmd.print_help().unwrap();
        println!(); // Add newline after help
        return Ok(());
    }

    // Validate arguments first
    args.validate()?;

    // Handle info commands that don't need config first
    if args.help_topic.is_some() {
        if let Some(topic) = &args.help_topic {
            args.print_help_topic(topic);
            return Ok(());
        }
    }

    let config = Config::load(&args.config)?;

    setup_logging(
        args.get_log_level(&config.logging.level),
        config.logging.show_timestamps,
        config.logging.colored_output && args.should_use_color(),
    )?;

    // Handle info commands that need config
    if handle_commands(&args, &config).await? {
        return Ok(()); // Info command was handled
    }

    // Handle encoding
    if args.should_encode() {
        handle_encoding(&args, &config).await
    } else {
        Ok(())
    }
}

async fn handle_encoding(args: &CliArgs, config: &Config) -> Result<()> {
    let ffmpeg = FfmpegWrapper::new(config.tools.ffmpeg.clone(), config.tools.ffprobe.clone());

    ffmpeg
        .check_availability()
        .await
        .map_err(|e| Error::ffmpeg(format!("FFmpeg tools not available: {}", e)))?;

    // Initialize stream preservation
    let stream_preservation = StreamPreservation::new(ffmpeg.clone());

    if args.input.is_empty() {
        return Err(Error::validation(
            "At least one input path is required for encoding".to_string(),
        ));
    }

    // Collect all video files from all input paths
    let mut all_video_files = Vec::new();
    for input_path in &args.input {
        let mut files = find_video_files(input_path)?;
        all_video_files.append(&mut files);
    }

    let video_files = all_video_files;
    info!("Found {} video file(s) to process", video_files.len());

    let mut profile_manager = ProfileManager::new();
    profile_manager.load_profiles(config.profiles.clone())?;

    let mut successful_files = 0;
    let mut failed_files = Vec::new();

    for (index, input_path) in video_files.iter().enumerate() {
        info!(
            "Processing file {}/{}: {}",
            index + 1,
            video_files.len(),
            input_path.display()
        );

        // Check if file exists before processing
        if !input_path.exists() {
            let error_msg = format!("File not found: {}", input_path.display());
            tracing::warn!("{}", error_msg);
            failed_files.push((input_path.clone(), error_msg));
            continue;
        }

        let output_path = if let Some(output) = &args.output {
            if video_files.len() > 1 {
                let parent = output.parent().unwrap_or(output);
                generate_uuid_filename(input_path, Some(parent))
            } else {
                output.clone()
            }
        } else {
            generate_uuid_filename(input_path, None::<&std::path::Path>)
        };

        match process_single_file(
            &ffmpeg,
            &stream_preservation,
            args,
            config,
            &mut profile_manager,
            input_path,
            &output_path,
        )
        .await
        {
            Ok(()) => {
                successful_files += 1;
                info!("✓ Successfully processed: {}", input_path.display());
            }
            Err(e) => {
                let error_msg = format!("Failed to process {}: {}", input_path.display(), e);
                tracing::error!("{}", error_msg);
                failed_files.push((input_path.clone(), error_msg));
            }
        }
    }

    // Report processing summary
    if video_files.len() > 1 {
        info!(
            "Processing complete: {} successful, {} failed",
            successful_files,
            failed_files.len()
        );

        if !failed_files.is_empty() {
            info!("Failed files:");
            for (path, error) in &failed_files {
                info!("  - {}: {}", path.display(), error);
            }
        }
    }

    // Only return error if all files failed
    if successful_files == 0 && !failed_files.is_empty() {
        return Err(Error::encoding("All files failed to process".to_string()));
    }

    Ok(())
}

async fn process_single_file(
    ffmpeg: &FfmpegWrapper,
    stream_preservation: &StreamPreservation,
    args: &CliArgs,
    config: &Config,
    profile_manager: &mut ProfileManager,
    input_path: &std::path::Path,
    output_path: &std::path::Path,
) -> Result<()> {
    info!("Getting video metadata for: {}", input_path.display());
    let metadata = ffmpeg.get_video_metadata(input_path).await?;

    // Comprehensive content analysis using UnifiedContentManager
    info!("Running comprehensive content analysis...");
    let content_manager = UnifiedContentManager::new(
        config.analysis.hdr.clone().unwrap_or_default(),
        config.analysis.dolby_vision.clone(),
        config.analysis.hdr10_plus.clone(),
        config.tools.hdr10plus_tool.clone(),
    );

    let content_analysis = content_manager.analyze_content(ffmpeg, input_path).await?;

    // Initialize metadata workflow manager for external tool integration
    info!("Initializing metadata workflow manager...");
    let metadata_workflow = MetadataWorkflowManager::new(config).await?;

    // Extract external metadata if tools are available
    let extracted_metadata = metadata_workflow
        .extract_metadata(
            input_path,
            &content_analysis.recommended_approach,
            &content_analysis.dolby_vision,
            &content_analysis.hdr_analysis,
        )
        .await?;

    // Log comprehensive analysis results
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
            if let Some(ref transfer) = metadata.transfer_function {
                info!("  Transfer Function: {}", transfer);
            }
            if let Some(ref primaries) = metadata.color_primaries {
                info!("  Color Primaries: {}", primaries);
            }
        }
        ContentEncodingApproach::DolbyVision(dv_info) => {
            info!("DOLBY VISION CONTENT DETECTED");
            info!("  Profile: {}", dv_info.profile.as_str());
            info!("  RPU Present: {}", dv_info.rpu_present);
            if let Some(ref color_space) = metadata.color_space {
                info!("  Color Space: {}", color_space);
            }
        }
        ContentEncodingApproach::DolbyVisionWithHDR10Plus(dv_info, hdr_result) => {
            info!("DUAL FORMAT CONTENT DETECTED: DOLBY VISION + HDR10+");
            info!("  Dolby Vision Profile: {}", dv_info.profile.as_str());
            info!("  RPU Present: {}", dv_info.rpu_present);
            info!("  HDR10+ Format: {:?}", hdr_result.metadata.format);
            if let Some(ref color_space) = metadata.color_space {
                info!("  Color Space: {}", color_space);
            }
            if let Some(ref transfer) = metadata.transfer_function {
                info!("  Transfer Function: {}", transfer);
            }
        }
    }

    let selected_profile = if args.profile == "auto" {
        info!("Auto-selecting profile based on content analysis...");
        select_profile_automatically(&metadata, profile_manager).await?
    } else {
        profile_manager
            .get_profile(&args.profile)
            .ok_or_else(|| Error::profile(format!("Profile '{}' not found", args.profile)))?
            .clone()
    };

    info!(
        "Selected profile: {} - {}",
        args.profile, selected_profile.title
    );

    // Create per-file logger
    let file_logger = FileLogger::new(output_path)?;

    // Use adjustments from comprehensive content analysis
    let adaptive_crf =
        selected_profile.base_crf + content_analysis.encoding_adjustments.crf_adjustment;
    let adaptive_bitrate = ((selected_profile.base_bitrate as f32)
        * content_analysis.encoding_adjustments.bitrate_multiplier)
        as u32;

    // Log content-specific parameter adjustments
    match &content_analysis.recommended_approach {
        ContentEncodingApproach::SDR => {
            info!(
                "Using standard encoding parameters (SDR): CRF={:.1}, Bitrate={}kbps",
                adaptive_crf, adaptive_bitrate
            );
        }
        ContentEncodingApproach::HDR(_) => {
            info!("HDR PARAMETER ADJUSTMENTS:");
            info!(
                "  Base CRF: {} -> Adjusted CRF: {:.1} (+{:.1})",
                selected_profile.base_crf,
                adaptive_crf,
                content_analysis.encoding_adjustments.crf_adjustment
            );
            info!(
                "  Base Bitrate: {} -> HDR Bitrate: {} ({:.1}x multiplier)",
                selected_profile.base_bitrate,
                adaptive_bitrate,
                content_analysis.encoding_adjustments.bitrate_multiplier
            );
        }
        ContentEncodingApproach::DolbyVision(_) => {
            info!("DOLBY VISION PARAMETER ADJUSTMENTS:");
            info!(
                "  Base CRF: {} -> Adjusted CRF: {:.1} (+{:.1})",
                selected_profile.base_crf,
                adaptive_crf,
                content_analysis.encoding_adjustments.crf_adjustment
            );
            info!(
                "  Base Bitrate: {} -> DV Bitrate: {} ({:.1}x multiplier)",
                selected_profile.base_bitrate,
                adaptive_bitrate,
                content_analysis.encoding_adjustments.bitrate_multiplier
            );
        }
        ContentEncodingApproach::DolbyVisionWithHDR10Plus(_, _) => {
            info!("DUAL FORMAT PARAMETER ADJUSTMENTS (ULTRA-CONSERVATIVE):");
            info!(
                "  Base CRF: {} -> Adjusted CRF: {:.1} (+{:.1})",
                selected_profile.base_crf,
                adaptive_crf,
                content_analysis.encoding_adjustments.crf_adjustment
            );
            info!(
                "  Base Bitrate: {} -> Dual Format Bitrate: {} ({:.1}x multiplier)",
                selected_profile.base_bitrate,
                adaptive_bitrate,
                content_analysis.encoding_adjustments.bitrate_multiplier
            );
        }
    }

    // Show x265 parameter preview based on content analysis
    let is_advanced_content = !matches!(
        content_analysis.recommended_approach,
        ContentEncodingApproach::SDR
    );
    let x265_params_preview = selected_profile.build_x265_params_string_with_hdr(
        None, // No mode-specific params for preview
        Some(is_advanced_content),
        metadata.color_space.as_ref(),
        metadata.transfer_function.as_ref(),
        metadata.color_primaries.as_ref(),
        metadata.master_display.as_ref(),
        metadata.max_cll.as_ref(),
    );

    if is_advanced_content {
        match &content_analysis.recommended_approach {
            ContentEncodingApproach::HDR(_) => {
                info!("HDR x265 parameters injected:");
            }
            ContentEncodingApproach::DolbyVision(_) => {
                info!("Dolby Vision x265 parameters injected:");
            }
            ContentEncodingApproach::DolbyVisionWithHDR10Plus(_, _) => {
                info!("Dual format (DV+HDR10+) x265 parameters injected:");
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
                    || p.contains("dolby-vision")
                    || p.contains("dhdr10-info")
            })
            .copied()
            .collect();
        if !special_params.is_empty() {
            for param in special_params {
                info!("  -> {}", param);
            }
        } else {
            info!("  -> No advanced format parameters added (using profile defaults)");
        }
    }

    // Crop detection with logging
    let (crop_values, crop_sample_timestamps, crop_analysis_result) =
        if config.analysis.crop_detection.enabled {
            use ffmpeg_autoencoder::analysis::CropDetector;
            let crop_detector = CropDetector::new(config.analysis.crop_detection.clone());

            let crop_analysis = crop_detector
                .detect_crop_values(
                    input_path,
                    metadata.duration,
                    metadata.width,
                    metadata.height,
                    is_advanced_content,
                )
                .await?;

            let sample_timestamps = config
                .analysis
                .crop_detection
                .get_sample_timestamps(metadata.duration);
            let crop_values = crop_analysis
                .crop_values
                .as_ref()
                .map(|cv| cv.to_ffmpeg_string());

            (crop_values, sample_timestamps, Some(crop_analysis))
        } else {
            (None, vec![], None)
        };

    let filter_chain = FilterBuilder::new(config)
        .with_deinterlace(args.deinterlace)?
        .with_denoise(args.denoise)
        .with_crop(crop_values.as_deref())?
        .build();

    let encoding_mode = EncodingMode::from_string(&args.mode)
        .ok_or_else(|| Error::encoding(format!("Invalid encoding mode: {}", args.mode)))?;

    // Generate stream mapping for preservation
    let stream_mapping = stream_preservation.analyze_streams(input_path).await?;

    // Log all encoding settings to file
    file_logger.log_encoding_settings(
        input_path,
        output_path,
        &args.profile,
        &selected_profile,
        &args.mode,
        adaptive_crf,
        adaptive_bitrate,
        Some(&filter_chain.to_string()),
        &format!("{:?}", stream_mapping),
    )?;

    // Log video analysis results including HDR/DV analysis
    file_logger.log_analysis_results(
        &metadata,
        None, // Grain level detection not implemented
        Some(&content_analysis),
    )?;

    // Log x265 parameters to file for reference
    file_logger.log_encoding_progress(&format!("x265 parameters: {}", x265_params_preview))?;

    // Log crop detection results
    let detection_method = if let Some(ref analysis) = crop_analysis_result {
        &analysis.detection_method
    } else if config.analysis.crop_detection.enabled {
        "automatic_detection"
    } else {
        "disabled"
    };

    file_logger.log_crop_detection_results(
        config.analysis.crop_detection.enabled,
        config.analysis.crop_detection.sample_count,
        &crop_sample_timestamps,
        crop_values.as_deref(),
        detection_method,
        config.analysis.crop_detection.sdr_crop_limit,
        config.analysis.crop_detection.hdr_crop_limit,
        is_advanced_content,
    )?;

    // Log additional crop analysis details if available
    if let Some(ref analysis) = crop_analysis_result {
        file_logger.log_encoding_progress(&format!(
            "Crop Analysis: {:.1}% confidence, {:.1}% pixel change, {} samples processed",
            analysis.confidence,
            analysis.pixel_change_percent,
            analysis.sample_results.len()
        ))?;
    }

    info!(
        "Starting {} encoding: {} -> {}",
        encoding_mode.as_str().to_uppercase(),
        input_path.display(),
        output_path.display()
    );

    file_logger.log_encoding_progress(&format!(
        "Starting {} encoding: {} -> {}",
        encoding_mode.as_str().to_uppercase(),
        input_path.file_name().unwrap_or_default().to_string_lossy(),
        output_path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
    ))?;

    // Determine if we need post-processing and use temporary output path if needed
    let needs_post_processing = metadata_workflow.needs_post_processing(&extracted_metadata);
    let actual_output_path = if needs_post_processing {
        info!("Post-processing required - using temporary output path");
        metadata_workflow.get_temp_output_path(output_path)
    } else {
        output_path.to_path_buf()
    };

    // Get external metadata parameters for x265
    let external_metadata_params =
        metadata_workflow.build_external_metadata_params(&extracted_metadata);
    let external_params_ref = if external_metadata_params.is_empty() {
        None
    } else {
        Some(external_metadata_params.as_slice())
    };

    let child = match encoding_mode {
        EncodingMode::CRF => {
            let encoder = CrfEncoder;
            encoder
                .encode(
                    ffmpeg,
                    input_path,
                    &actual_output_path,
                    &selected_profile,
                    &filter_chain,
                    &stream_mapping,
                    &metadata,
                    adaptive_crf,
                    adaptive_bitrate,
                    args.title.as_deref(),
                    Some(&file_logger),
                    external_params_ref,
                )
                .await?
        }
        EncodingMode::ABR => {
            let encoder = AbrEncoder;
            encoder
                .encode(
                    ffmpeg,
                    input_path,
                    &actual_output_path,
                    &selected_profile,
                    &filter_chain,
                    &stream_mapping,
                    &metadata,
                    adaptive_crf,
                    adaptive_bitrate,
                    args.title.as_deref(),
                    Some(&file_logger),
                    external_params_ref,
                )
                .await?
        }
        EncodingMode::CBR => {
            let encoder = CbrEncoder::new();
            encoder
                .encode(
                    ffmpeg,
                    input_path,
                    &actual_output_path,
                    &selected_profile,
                    &filter_chain,
                    &stream_mapping,
                    &metadata,
                    adaptive_crf,
                    adaptive_bitrate,
                    args.title.as_deref(),
                    Some(&file_logger),
                    external_params_ref,
                )
                .await?
        }
    };

    let start_time = std::time::Instant::now();

    // Initialize progress monitor with frame calculation
    let mut progress_monitor = ProgressMonitor::new(
        metadata.duration,
        metadata.fps,
        ffmpeg.clone(),
        encoding_mode,
    );
    let total_frames = if metadata.fps > 0.0 && metadata.duration > 0.0 {
        (metadata.duration * metadata.fps as f64) as u32
    } else {
        0
    };

    progress_monitor.set_message(&format!(
        "Encoding {} ({}x{}, {:.1}fps, {} frames)",
        input_path.file_name().unwrap_or_default().to_string_lossy(),
        metadata.width,
        metadata.height,
        metadata.fps,
        total_frames
    ));

    let status = progress_monitor.monitor_encoding(child).await?;
    let duration = start_time.elapsed();

    // Handle post-encoding metadata injection if needed
    if status.success() && needs_post_processing {
        info!("Starting post-encoding metadata injection...");
        metadata_workflow
            .inject_metadata(
                &actual_output_path,
                &output_path.to_path_buf(),
                &extracted_metadata,
            )
            .await?;
        info!("Post-encoding metadata injection completed!");
    }

    let output_size = std::fs::metadata(output_path).map(|m| m.len()).ok();
    let exit_code = status.code();

    if status.success() {
        // Log success to both console and file
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
        // Log failure to file before returning error
        file_logger.log_encoding_complete(false, duration, output_size, exit_code)?;

        return Err(Error::encoding(format!(
            "Encoding failed with exit code: {}",
            exit_code.unwrap_or(-1)
        )));
    }

    // Clean up temporary metadata files
    metadata_workflow.cleanup().await.unwrap_or_else(|e| {
        tracing::warn!("Failed to cleanup metadata files: {}", e);
    });

    // Clean up extracted metadata files explicitly
    extracted_metadata.cleanup();

    Ok(())
}

async fn select_profile_automatically(
    metadata: &ffmpeg_autoencoder::utils::ffmpeg::VideoMetadata,
    profile_manager: &ProfileManager,
) -> Result<ffmpeg_autoencoder::config::EncodingProfile> {
    let content_type = classify_content_from_metadata(metadata).await?;

    if let Some(profile) = profile_manager.recommend_profile_for_resolution(
        metadata.width,
        metadata.height,
        content_type,
    ) {
        Ok(profile.clone())
    } else {
        info!("No specific profile found for content type, using default 'movie' profile");
        profile_manager
            .get_profile("movie")
            .cloned()
            .ok_or_else(|| Error::profile("Default 'movie' profile not found"))
    }
}

async fn classify_content_from_metadata(
    metadata: &ffmpeg_autoencoder::utils::ffmpeg::VideoMetadata,
) -> Result<ffmpeg_autoencoder::config::ContentType> {
    use ffmpeg_autoencoder::config::ContentType;

    let _is_4k = metadata.width >= 3840 || metadata.height >= 2160;
    let bitrate_per_pixel =
        metadata.bitrate.unwrap_or(0) as f64 / (metadata.width as f64 * metadata.height as f64);

    if bitrate_per_pixel > 0.02 {
        Ok(ContentType::HeavyGrain)
    } else if bitrate_per_pixel > 0.015 {
        Ok(ContentType::LightGrain)
    } else {
        Ok(ContentType::Film)
    }
}
