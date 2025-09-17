use crate::encoding::EncodingMode;
use crate::utils::{FfmpegWrapper, Result};
use indicatif::{ProgressBar, ProgressStyle};
use std::time::{Duration, Instant};
use tokio::process::Child;

pub struct ProgressMonitor {
    progress_bar: ProgressBar,
    start_time: Instant,
    total_duration: f64,
    total_frames: Option<u32>,
    is_two_pass: bool,
    last_progress: f64,
    source_fps: f32,
}

impl ProgressMonitor {
    pub fn new(
        total_duration: f64,
        fps: f32,
        _ffmpeg: FfmpegWrapper,
        encoding_mode: EncodingMode,
    ) -> Self {
        let is_two_pass = matches!(encoding_mode, EncodingMode::ABR | EncodingMode::CBR);
        let progress_bar = ProgressBar::new(10000); // Use 10000 as max for 0.01% precision

        // Adjust progress bar template for two-pass encoding
        let template = if is_two_pass {
            "{spinner:.green} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {percent_precise:>5}% (Pass 2/2) | {msg}"
        } else {
            "{spinner:.green} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {percent_precise:>5}% | {msg}"
        };

        progress_bar.set_style(
            ProgressStyle::with_template(template)
                .unwrap()
                .with_key(
                    "percent_precise",
                    |state: &indicatif::ProgressState, w: &mut dyn std::fmt::Write| {
                        _ = write!(w, "{:>4.1}", state.fraction() * 100.0);
                    },
                )
                .progress_chars("█▉▊▋▌▍▎▏ "),
        );

        // Calculate total frames using duration × framerate
        let total_frames = if fps > 0.0 && total_duration > 0.0 {
            Some((total_duration * fps as f64) as u32)
        } else {
            None
        };

        Self {
            progress_bar,
            start_time: Instant::now(),
            total_duration,
            total_frames,
            is_two_pass,
            last_progress: 0.0,
            source_fps: fps,
        }
    }

    pub fn set_message(&self, message: &str) {
        self.progress_bar.set_message(message.to_string());
    }

    pub fn start_pass_one(&self) {
        if self.is_two_pass {
            self.progress_bar
                .set_message("Starting Pass 1/2 (analysis)...".to_string());
        }
    }

    pub fn start_pass_two(&self) {
        if self.is_two_pass {
            self.progress_bar
                .set_message("Starting Pass 2/2 (encoding)...".to_string());
        }
    }

    pub async fn monitor_encoding(&mut self, mut child: Child) -> Result<std::process::ExitStatus> {
        use std::path::Path;
        use tokio::time::{interval, Duration};

        // For two-pass encoding, reset start time when monitoring begins (Pass 2)
        if self.is_two_pass {
            self.start_time = Instant::now();
            self.last_progress = 0.0; // Reset progress for Pass 2
            self.start_pass_two();
        }

        // Get progress file path (same format as in encoding)
        let progress_file = format!("/tmp/ffmpeg_progress_{}.txt", std::process::id());

        // Monitor progress file for encoding updates
        let mut interval_timer = interval(Duration::from_millis(500));

        loop {
            interval_timer.tick().await;

            // Check if process is still running
            match child.try_wait()? {
                Some(status) => {
                    self.finish();
                    return Ok(status);
                }
                None => {
                    // Process still running, check progress file
                    if Path::new(&progress_file).exists() {
                        if let Ok(content) = tokio::fs::read_to_string(&progress_file).await {
                            if let Some(progress_info) = self.parse_progress_file(&content) {
                                self.update_progress(&progress_info);
                            }
                        }
                    }
                }
            }
        }
    }

    fn parse_progress_file(&self, content: &str) -> Option<crate::utils::ffmpeg::ProgressInfo> {
        let mut progress = crate::utils::ffmpeg::ProgressInfo {
            frame: None,
            fps: None,
            bitrate: None,
            total_size: None,
            time: 0.0,
            speed: None,
            progress_percentage: 0.0,
        };

        // Parse key=value lines from FFmpeg progress output
        let lines: Vec<&str> = content.lines().collect();
        let last_lines: Vec<&str> = lines.iter().rev().take(20).cloned().collect();

        for line in last_lines.iter().rev() {
            let line = line.trim();
            if line.contains('=') {
                let parts: Vec<&str> = line.splitn(2, '=').collect();
                if parts.len() == 2 {
                    let key = parts[0].trim();
                    let value = parts[1].trim();

                    match key {
                        "frame" => {
                            progress.frame = value.parse().ok();
                        }
                        "fps" => {
                            progress.fps = value.parse().ok();
                        }
                        "out_time_us" => {
                            if let Ok(time_us) = value.parse::<u64>() {
                                progress.time = time_us as f64 / 1_000_000.0; // Convert microseconds to seconds
                                if self.total_duration > 0.0 {
                                    progress.progress_percentage =
                                        ((progress.time / self.total_duration) * 100.0).min(100.0)
                                            as f32;
                                }
                            }
                        }
                        "speed" => {
                            // Remove 'x' suffix if present
                            let speed_str = value.trim_end_matches('x');
                            progress.speed = speed_str.parse().ok();
                        }
                        "total_size" => {
                            progress.total_size = value.parse().ok();
                        }
                        _ => {} // Ignore other keys
                    }
                }
            }
        }

        // Only return if we have meaningful progress data
        if progress.frame.is_some() || progress.time > 0.0 {
            Some(progress)
        } else {
            None
        }
    }

    fn update_progress(&mut self, info: &crate::utils::ffmpeg::ProgressInfo) {
        // Always use time-based progress for consistency, especially with complex filters
        let mut current_progress = info.progress_percentage as f64 / 100.0;
        let time_based_progress = current_progress;

        // Frame-based progress can be unreliable with deinterlacing/complex filters
        // Only use frame-based as a fallback if time-based isn't working
        let mut frame_based_progress = None;
        if current_progress <= 0.0 {
            if let (Some(current_frame), Some(total_frames)) = (info.frame, self.total_frames) {
                if current_frame > 0 && total_frames > 0 {
                    let frame_progress = current_frame as f64 / total_frames as f64;
                    if frame_progress > 0.0 && frame_progress <= 1.0 {
                        current_progress = frame_progress;
                        frame_based_progress = Some(frame_progress);
                    }
                }
            }
        }

        // Debug logging to track progress calculation issues
        if tracing::enabled!(tracing::Level::DEBUG) && current_progress > 0.01 {
            let debug_msg = if let Some(fb_prog) = frame_based_progress {
                format!(
                    "Progress debug: time={:.1}% frame={:.1}% using=frame final={:.1}% last={:.1}%",
                    time_based_progress * 100.0,
                    fb_prog * 100.0,
                    current_progress * 100.0,
                    self.last_progress * 100.0
                )
            } else {
                format!(
                    "Progress debug: time={:.1}% using=time final={:.1}% last={:.1}%",
                    time_based_progress * 100.0,
                    current_progress * 100.0,
                    self.last_progress * 100.0
                )
            };

            // Only log if progress changes significantly to avoid spam
            if (current_progress - self.last_progress).abs() > 0.02 {
                tracing::debug!("{}", debug_msg);
            }
        }

        // Prevent backwards progress movement - only allow forward progress
        current_progress = current_progress.max(self.last_progress);

        // Ensure progress doesn't exceed 100%
        current_progress = current_progress.min(1.0);

        // Store the current progress for next iteration
        self.last_progress = current_progress;

        let position = (current_progress * 10000.0) as u64;
        self.progress_bar.set_position(position);

        // Update message with current stats
        let mut message_parts = vec![];

        // Build a clean, compact status message
        if let Some(encoding_fps) = info.fps {
            message_parts.push(format!("{:.1}fps", encoding_fps));

            // Calculate actual speed multiplier from source FPS
            if self.source_fps > 0.0 {
                let actual_speed_multiplier = encoding_fps / self.source_fps;
                message_parts.push(format!("{:.1}x", actual_speed_multiplier));
            }
        } else if let Some(speed) = info.speed {
            // Fallback to FFmpeg's speed value if no FPS available
            message_parts.push(format!("{:.1}x", speed));
        }

        // Add size estimation if we have enough data
        if current_progress > 0.01 {
            if let Some(current_size) = info.total_size {
                let estimated_final_size = (current_size as f64 / current_progress) as u64;
                message_parts.push(format!("~{}", format_size(estimated_final_size)));
            }
        }

        // Enhanced ETA calculation with multiple methods
        if current_progress > 0.005 {
            let elapsed = self.start_time.elapsed().as_secs_f64();

            // Primary method: Progress-based ETA (most stable)
            let mut eta_seconds = (elapsed / current_progress) - elapsed;

            // Use frame-based method as fallback/validation if available
            if let (Some(current_fps), Some(total_frames)) = (info.fps, self.total_frames) {
                if current_fps > 0.1 && total_frames > 100 && current_progress > 0.01 {
                    let remaining_frames = (total_frames as f64 * (1.0 - current_progress)) as u32;
                    if remaining_frames > 0 {
                        let eta_frame = remaining_frames as f64 / current_fps as f64;
                        // Use frame-based ETA if it's reasonable and progress-based seems off
                        if eta_frame > 0.0 && eta_frame < (48.0 * 3600.0) {
                            // Prefer frame-based for very early stages or if time-based seems unreasonable
                            if current_progress < 0.02
                                || !(5.0..=(24.0 * 3600.0)).contains(&eta_seconds)
                            {
                                eta_seconds = eta_frame;
                            }
                        }
                    }
                }
            }

            // Apply speed adjustment if reasonable
            if let Some(speed) = info.speed {
                if speed > 0.5 && speed < 3.0 {
                    let eta_speed_adjusted = eta_seconds / speed as f64;
                    // Only use speed adjustment if the result is reasonable
                    if eta_speed_adjusted > 0.0 && eta_speed_adjusted < (eta_seconds * 2.0) {
                        eta_seconds = eta_speed_adjusted;
                    }
                }
            }

            // Sanity check: cap at 24 hours, minimum 5 seconds
            eta_seconds = eta_seconds.clamp(5.0, 24.0 * 3600.0);

            if eta_seconds > 0.0 {
                let eta = Duration::from_secs_f64(eta_seconds);
                message_parts.push(format!("ETA {}", format_duration(eta)));
            }
        }

        if !message_parts.is_empty() {
            self.set_message(&message_parts.join(" • "));
        }
    }

    fn finish(&self) {
        let duration = self.start_time.elapsed();
        self.progress_bar.set_position(10000);
        self.progress_bar
            .finish_with_message(format!("Completed in {}", format_duration(duration)));

        // Cleanup progress file
        let progress_file = format!("/tmp/ffmpeg_progress_{}.txt", std::process::id());
        let _ = std::fs::remove_file(&progress_file);
    }
}

fn format_duration(duration: Duration) -> String {
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

fn format_size(bytes: u64) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];
    let mut size = bytes as f64;
    let mut unit_index = 0;

    while size >= 1024.0 && unit_index < UNITS.len() - 1 {
        size /= 1024.0;
        unit_index += 1;
    }

    if unit_index == 0 {
        format!("{} {}", bytes, UNITS[unit_index])
    } else {
        format!("{:.1} {}", size, UNITS[unit_index])
    }
}
