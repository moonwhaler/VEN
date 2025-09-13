use clap::Parser;
use tracing::info;

use ffmpeg_autoencoder::{
    cli::{CliArgs, handle_commands},
    config::{Config, ProfileManager},
    utils::{setup_logging, find_video_files, generate_uuid_filename, FfmpegWrapper, Result, Error, FileLogger, ProgressMonitor},
    encoding::{EncodingMode, FilterBuilder, CrfEncoder, AbrEncoder, CbrEncoder, modes::Encoder},
    stream::preservation::StreamPreservation,
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
        args.get_log_level(),
        config.logging.show_timestamps,
        config.logging.colored_output && args.should_use_color(),
    )?;

    // Handle info commands that need config
    if handle_commands(&args, &config).await? {
        return Ok(()); // Info command was handled
    }

    // Handle encoding
    handle_encoding(&args, &config).await
}

async fn handle_encoding(args: &CliArgs, config: &Config) -> Result<()> {
    let ffmpeg = FfmpegWrapper::new(
        config.tools.ffmpeg.clone(),
        config.tools.ffprobe.clone(),
    );

    ffmpeg.check_availability().await
        .map_err(|e| Error::ffmpeg(format!("FFmpeg tools not available: {}", e)))?;

    // Initialize stream preservation
    let stream_preservation = StreamPreservation::new(ffmpeg.clone());

    if args.input.is_empty() {
        return Err(Error::validation("At least one input path is required for encoding".to_string()));
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

    for (index, input_path) in video_files.iter().enumerate() {
        info!("Processing file {}/{}: {}", index + 1, video_files.len(), input_path.display());

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

        process_single_file(
            &ffmpeg, 
            &stream_preservation,
            args, 
            config, 
            &mut profile_manager, 
            input_path, 
            &output_path
        ).await?;
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

    let selected_profile = if args.profile == "auto" {
        info!("Auto-selecting profile based on content analysis...");
        select_profile_automatically(&metadata, profile_manager).await?
    } else {
        profile_manager.get_profile(&args.profile)
            .ok_or_else(|| Error::profile(format!("Profile '{}' not found", args.profile)))?
            .clone()
    };

    info!("Selected profile: {} - {}", args.profile, selected_profile.title);

    // Create per-file logger
    let file_logger = FileLogger::new(output_path)?;
    
    let adaptive_crf = selected_profile.calculate_adaptive_crf(0.0, metadata.is_hdr, config.analysis.hdr_detection.crf_adjustment);
    let adaptive_bitrate = selected_profile.calculate_adaptive_bitrate(1.0, metadata.is_hdr);

    // Crop detection with logging
    let (crop_values, crop_sample_timestamps, crop_analysis_result) = if let Some(crop) = &args.crop {
        (Some(crop.clone()), vec![-1.0], None) // -1.0 indicates manual override
    } else if config.analysis.crop_detection.enabled {
        use ffmpeg_autoencoder::analysis::CropDetector;
        let crop_detector = CropDetector::new(config.analysis.crop_detection.clone());
        
        let crop_analysis = crop_detector.detect_crop_values(
            input_path, 
            metadata.duration, 
            metadata.width, 
            metadata.height, 
            metadata.is_hdr
        ).await?;
        
        let sample_timestamps = config.analysis.crop_detection.get_sample_timestamps(metadata.duration);
        let crop_values = crop_analysis.crop_values.as_ref().map(|cv| cv.to_ffmpeg_string());
        
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
    
    // Log video analysis results
    file_logger.log_analysis_results(
        &metadata,
        None, // TODO: Add grain level when available  
    )?;
    
    // Log crop detection results
    let detection_method = if args.crop.is_some() {
        "manual_override"
    } else if let Some(ref analysis) = crop_analysis_result {
        &analysis.detection_method
    } else if config.analysis.crop_detection.enabled {
        "automatic_detection"
    } else {
        "disabled"
    };
    
    file_logger.log_crop_detection_results(
        config.analysis.crop_detection.enabled || args.crop.is_some(),
        config.analysis.crop_detection.sample_count,
        &crop_sample_timestamps,
        crop_values.as_deref(),
        detection_method,
        config.analysis.crop_detection.sdr_crop_limit,
        config.analysis.crop_detection.hdr_crop_limit,
        metadata.is_hdr,
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

    info!("Starting {} encoding: {} -> {}",
          encoding_mode.as_str().to_uppercase(),
          input_path.display(),
          output_path.display());
          
    file_logger.log_encoding_progress(&format!(
        "Starting {} encoding: {} -> {}",
        encoding_mode.as_str().to_uppercase(),
        input_path.file_name().unwrap_or_default().to_string_lossy(),
        output_path.file_name().unwrap_or_default().to_string_lossy()
    ))?;

    let child = match encoding_mode {
        EncodingMode::CRF => {
            let encoder = CrfEncoder;
            encoder.encode(
                ffmpeg,
                input_path,
                output_path,
                &selected_profile,
                &filter_chain,
                &stream_mapping,
                &metadata,
                adaptive_crf,
                adaptive_bitrate,
                args.title.as_deref(),
            ).await?
        }
        EncodingMode::ABR => {
            let encoder = AbrEncoder;
            encoder.encode(
                ffmpeg,
                input_path,
                output_path,
                &selected_profile,
                &filter_chain,
                &stream_mapping,
                &metadata,
                adaptive_crf,
                adaptive_bitrate,
                args.title.as_deref(),
            ).await?
        }
        EncodingMode::CBR => {
            let encoder = CbrEncoder::new();
            encoder.encode(
                ffmpeg,
                input_path,
                output_path,
                &selected_profile,
                &filter_chain,
                &stream_mapping,
                &metadata,
                adaptive_crf,
                adaptive_bitrate,
                args.title.as_deref(),
            ).await?
        }
    };

    let start_time = std::time::Instant::now();
    
    // Initialize progress monitor with frame calculation
    let mut progress_monitor = ProgressMonitor::new(metadata.duration, metadata.fps, ffmpeg.clone());
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

    let output_size = std::fs::metadata(output_path).map(|m| m.len()).ok();
    let exit_code = status.code();
    
    if status.success() {
        // Log success to both console and file
        if let Some(size) = output_size {
            info!("Encoding completed successfully in {:.2}s, output size: {:.2} MB",
                  duration.as_secs_f64(),
                  size as f64 / 1_048_576.0);
        } else {
            info!("Encoding completed successfully in {:.2}s", duration.as_secs_f64());
        }
        
        file_logger.log_encoding_complete(true, duration, output_size, exit_code)?;
        info!("Encoding log saved to: {}", file_logger.get_log_path().display());
    } else {
        // Log failure to file before returning error
        file_logger.log_encoding_complete(false, duration, output_size, exit_code)?;
        
        return Err(Error::encoding(format!(
            "Encoding failed with exit code: {}",
            exit_code.unwrap_or(-1)
        )));
    }

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
        profile_manager.get_profile("movie")
            .cloned()
            .ok_or_else(|| Error::profile("Default 'movie' profile not found"))
    }
}

async fn classify_content_from_metadata(
    metadata: &ffmpeg_autoencoder::utils::ffmpeg::VideoMetadata,
) -> Result<ffmpeg_autoencoder::config::ContentType> {
    use ffmpeg_autoencoder::config::ContentType;
    
    let _is_4k = metadata.width >= 3840 || metadata.height >= 2160;
    let bitrate_per_pixel = metadata.bitrate.unwrap_or(0) as f64 / 
                           (metadata.width as f64 * metadata.height as f64);

    if bitrate_per_pixel > 0.02 {
        Ok(ContentType::HeavyGrain)
    } else if bitrate_per_pixel > 0.015 {
        Ok(ContentType::LightGrain)
    } else {
        Ok(ContentType::Film)
    }
}