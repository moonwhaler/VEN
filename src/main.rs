use clap::Parser;
use tracing::info;

use ffmpeg_autoencoder::{
    cli::{handle_commands, CliArgs},
    config::{Config, PreviewProfileManager, ProfileManager},
    preview::{PreviewConfig, PreviewMode, PreviewProcessor},
    processing::VideoProcessor,
    stream::preservation::StreamPreservation,
    utils::{
        find_video_files, generate_uuid_filename, setup_logging, Error, FfmpegWrapper, Result,
    },
};

#[tokio::main]
async fn main() -> Result<()> {
    let args = CliArgs::parse();

    if !args.is_info_command() && args.input.is_empty() {
        use clap::CommandFactory;
        let mut cmd = CliArgs::command();
        cmd.print_help().unwrap();
        println!();
        return Ok(());
    }

    args.validate()?;

    let config = Config::load_with_discovery(args.config.as_deref())?;

    setup_logging(
        args.get_log_level(&config.logging.level),
        config.logging.show_timestamps,
        config.logging.colored_output,
    )?;

    if handle_commands(&args, &config).await? {
        return Ok(());
    }

    if args.should_encode() {
        handle_encoding(&args, &config).await
    } else if args.should_preview() {
        handle_preview(&args, &config).await
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

    let stream_preservation = StreamPreservation::new(ffmpeg.clone());

    if args.input.is_empty() {
        return Err(Error::validation(
            "At least one input path is required for encoding".to_string(),
        ));
    }

    let mut all_video_files = Vec::new();
    for input_path in &args.input {
        let mut files = find_video_files(input_path)?;
        all_video_files.append(&mut files);
    }

    let video_files = all_video_files;
    info!("Found {} video file(s) to process", video_files.len());

    let mut profile_manager = ProfileManager::new();
    profile_manager.load_profiles(config.profiles.clone())?;

    if args.profile != "auto" && profile_manager.get_profile(&args.profile).is_none() {
        let available_profiles: Vec<String> = profile_manager
            .list_profiles()
            .into_iter()
            .cloned()
            .collect();
        let mut all_valid_profiles = vec!["auto".to_string()];
        all_valid_profiles.extend(available_profiles);
        all_valid_profiles.sort();

        return Err(Error::validation(format!(
            "Invalid profile: {} (valid profiles: {})",
            args.profile,
            all_valid_profiles.join(", ")
        )));
    }

    let mut successful_files = 0;
    let mut failed_files = Vec::new();

    for (index, input_path) in video_files.iter().enumerate() {
        info!(
            "Processing file {}/{}: {}",
            index + 1,
            video_files.len(),
            input_path.display()
        );

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
    let mut processor = VideoProcessor::new(
        ffmpeg,
        stream_preservation,
        args,
        config,
        profile_manager,
        input_path,
        output_path,
    )?;
    processor.run().await
}

async fn handle_preview(args: &CliArgs, config: &Config) -> Result<()> {
    let ffmpeg = FfmpegWrapper::new(config.tools.ffmpeg.clone(), config.tools.ffprobe.clone());

    ffmpeg
        .check_availability()
        .await
        .map_err(|e| Error::ffmpeg(format!("FFmpeg tools not available: {}", e)))?;

    if args.input.is_empty() {
        return Err(Error::validation(
            "At least one input path is required for preview mode".to_string(),
        ));
    }

    // Get the first input file (preview mode only supports single file)
    let input_path = &args.input[0];
    if !input_path.is_file() {
        return Err(Error::validation(format!(
            "Preview mode requires a single video file as input, not a directory: {}",
            input_path.display()
        )));
    }

    // Load profile manager
    let mut profile_manager = ProfileManager::new();
    profile_manager.load_profiles(config.profiles.clone())?;

    // Determine which profiles to use
    let profile_names = get_preview_profile_names(args, config, &profile_manager)?;

    info!("Preview mode enabled - testing {} profile(s)", profile_names.len());

    // Determine preview mode
    let preview_mode = if let Some(timestamp) = args.preview_time {
        PreviewMode::Image { timestamp }
    } else if let Some((start, end)) = args.parse_preview_range() {
        PreviewMode::VideoSegment { start, end }
    } else {
        return Err(Error::validation(
            "Preview mode requires either --preview-time or --preview-range".to_string(),
        ));
    };

    // Create preview config
    let preview_config = PreviewConfig {
        mode: preview_mode,
        profile_names,
    };

    // Determine output directory from -o parameter if provided
    let output_dir = args.output.as_ref().and_then(|p| {
        if p.is_dir() {
            Some(p.as_path())
        } else {
            p.parent()
        }
    });

    // Create preview processor and generate previews
    let processor = PreviewProcessor::new(&ffmpeg, config, &profile_manager, input_path, output_dir, preview_config);
    let _results = processor.generate_previews().await?;

    Ok(())
}

fn get_preview_profile_names(
    args: &CliArgs,
    config: &Config,
    profile_manager: &ProfileManager,
) -> Result<Vec<String>> {
    if let Some(preview_profile_name) = &args.preview_profile {
        // Use custom preview profile group from config
        let preview_manager = PreviewProfileManager::new(config.preview_profiles.clone())?;
        let preview_profile = preview_manager.get_profile(preview_profile_name)?;

        // Validate that all profiles in the preview profile exist
        for profile_name in &preview_profile.profiles {
            if profile_manager.get_profile(profile_name).is_none() {
                return Err(Error::validation(format!(
                    "Preview profile '{}' references non-existent encoding profile '{}'",
                    preview_profile_name, profile_name
                )));
            }
        }

        Ok(preview_profile.profiles.clone())
    } else if args.profile != "auto" {
        // Use single specified profile
        if profile_manager.get_profile(&args.profile).is_none() {
            return Err(Error::validation(format!(
                "Profile '{}' not found",
                args.profile
            )));
        }
        Ok(vec![args.profile.clone()])
    } else {
        // Use ALL available profiles
        let mut all_profiles: Vec<String> = profile_manager
            .list_profiles()
            .into_iter()
            .cloned()
            .collect();
        all_profiles.sort();
        Ok(all_profiles)
    }
}
