use crate::utils::{FfmpegWrapper, Result};
use indicatif::{ProgressBar, ProgressStyle};
use std::time::{Duration, Instant};
use tokio::process::Child;

pub struct ProgressMonitor {
    progress_bar: ProgressBar,
    start_time: Instant,
    total_duration: f64,
    total_frames: Option<u32>,
}

impl ProgressMonitor {
    pub fn new(total_duration: f64, fps: f32, _ffmpeg: FfmpegWrapper) -> Self {
        let progress_bar = ProgressBar::new(10000); // Use 10000 as max for 0.01% precision

        progress_bar.set_style(
            ProgressStyle::with_template(
                "{spinner:.green} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {percent:>3}% | {msg}"
            )
            .unwrap()
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
        }
    }

    pub fn set_message(&self, message: &str) {
        self.progress_bar.set_message(message.to_string());
    }

    pub async fn monitor_encoding(&mut self, mut child: Child) -> Result<std::process::ExitStatus> {
        use std::path::Path;
        use tokio::time::{interval, Duration};

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
        // Calculate progress using both time and frame methods
        let mut current_progress = info.progress_percentage as f64 / 100.0;

        // Frame-based progress calculation (more reliable for some content)
        if let (Some(current_frame), Some(total_frames)) = (info.frame, self.total_frames) {
            if current_frame > 0 && total_frames > 0 {
                let frame_progress = current_frame as f64 / total_frames as f64;
                // Prefer frame-based if it's available and reasonable
                if frame_progress > 0.0 && frame_progress <= 1.0 {
                    current_progress = frame_progress;
                }
            }
        }

        // Ensure progress doesn't exceed 100%
        current_progress = current_progress.min(1.0);

        let position = (current_progress * 10000.0) as u64;
        self.progress_bar.set_position(position);

        // Update message with current stats
        let mut message_parts = vec![];

        // Build a clean, compact status message
        if let Some(fps) = info.fps {
            message_parts.push(format!("{:.1}fps", fps));
        }

        if let Some(speed) = info.speed {
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
