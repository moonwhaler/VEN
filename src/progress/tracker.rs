use indicatif::{ProgressBar, ProgressStyle};
use std::time::{Duration, Instant};
use tracing::{debug, warn, info};
use crate::utils::{Result};
use regex::Regex;
use once_cell::sync::Lazy;

static PROGRESS_REGEX: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"time=(\d{2}):(\d{2}):(\d{2})\.(\d{2})").unwrap()
});

static FRAME_REGEX: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"frame=\s*(\d+)").unwrap()
});

static SPEED_REGEX: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"speed=\s*([0-9.]+)x").unwrap()
});

static SIZE_REGEX: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"size=\s*(\d+)kB").unwrap()
});

static FPS_REGEX: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"fps=\s*([0-9.]+)").unwrap()
});

static BITRATE_REGEX: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"bitrate=\s*([0-9.]+)kbits/s").unwrap()
});

#[derive(Debug, Clone)]
pub struct ProgressMetrics {
    pub current_frame: u32,
    pub total_frames: u32,
    pub current_time: f64,
    pub total_duration: f64,
    pub fps: Option<f32>,
    pub speed: Option<f32>,
    pub bitrate: Option<f32>,
    pub size_kb: Option<u64>,
    pub progress_percentage: f32,
    pub estimated_final_size: Option<u64>,
}

impl ProgressMetrics {
    pub fn new(total_duration: f64, fps: f32) -> Self {
        let total_frames = (total_duration * fps as f64) as u32;
        
        Self {
            current_frame: 0,
            total_frames,
            current_time: 0.0,
            total_duration,
            fps: Some(fps),
            speed: None,
            bitrate: None,
            size_kb: None,
            progress_percentage: 0.0,
            estimated_final_size: None,
        }
    }
    
    pub fn update_from_ffmpeg_output(&mut self, line: &str) -> bool {
        let mut updated = false;
        
        // Extract frame number
        if let Some(captures) = FRAME_REGEX.captures(line) {
            if let Ok(frame) = captures[1].parse::<u32>() {
                self.current_frame = frame;
                updated = true;
            }
        }
        
        // Extract current time
        if let Some(captures) = PROGRESS_REGEX.captures(line) {
            let hours: u32 = captures[1].parse().unwrap_or(0);
            let minutes: u32 = captures[2].parse().unwrap_or(0);
            let seconds: u32 = captures[3].parse().unwrap_or(0);
            let centiseconds: u32 = captures[4].parse().unwrap_or(0);
            
            self.current_time = (hours * 3600 + minutes * 60 + seconds) as f64 + 
                               (centiseconds as f64 / 100.0);
            updated = true;
        }
        
        // Extract encoding speed
        if let Some(captures) = SPEED_REGEX.captures(line) {
            if let Ok(speed) = captures[1].parse::<f32>() {
                self.speed = Some(speed);
                updated = true;
            }
        }
        
        // Extract current FPS
        if let Some(captures) = FPS_REGEX.captures(line) {
            if let Ok(fps) = captures[1].parse::<f32>() {
                self.fps = Some(fps);
                updated = true;
            }
        }
        
        // Extract bitrate
        if let Some(captures) = BITRATE_REGEX.captures(line) {
            if let Ok(bitrate) = captures[1].parse::<f32>() {
                self.bitrate = Some(bitrate);
                updated = true;
            }
        }
        
        // Extract file size
        if let Some(captures) = SIZE_REGEX.captures(line) {
            if let Ok(size) = captures[1].parse::<u64>() {
                self.size_kb = Some(size);
                
                // Estimate final size based on progress
                if self.current_time > 0.0 && self.total_duration > 0.0 {
                    let progress_ratio = self.current_time / self.total_duration;
                    if progress_ratio > 0.01 { // Only estimate after 1% progress
                        self.estimated_final_size = Some((size as f64 / progress_ratio) as u64);
                    }
                }
                updated = true;
            }
        }
        
        // Calculate progress percentage using both frame and time methods
        if updated {
            let time_progress = if self.total_duration > 0.0 {
                (self.current_time / self.total_duration * 100.0).min(100.0)
            } else {
                0.0
            };
            
            let frame_progress = if self.total_frames > 0 {
                (self.current_frame as f64 / self.total_frames as f64 * 100.0).min(100.0)
            } else {
                0.0
            };
            
            // Use frame-based progress when available, fall back to time-based
            self.progress_percentage = if self.total_frames > 0 && frame_progress > 0.0 {
                frame_progress as f32
            } else {
                time_progress as f32
            };
        }
        
        updated
    }
}

pub struct EnhancedProgressTracker {
    bar: ProgressBar,
    metrics: ProgressMetrics,
    start_time: Instant,
    last_update: Instant,
    last_frame: u32,
    last_time: f64,
    stall_detection_enabled: bool,
    stall_threshold: Duration,
    update_interval: Duration,
    show_eta: bool,
    show_file_size: bool,
}

impl EnhancedProgressTracker {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        total_duration: f64, 
        fps: f32,
        description: &str,
        stall_detection_enabled: bool,
        stall_threshold_seconds: u64,
        update_interval_ms: u64,
        show_eta: bool,
        show_file_size: bool,
    ) -> Self {
        let bar = ProgressBar::new(10000); // Use 10000 for higher precision (0.01%)
        
        // Enhanced progress bar template matching bash script
        let template = if show_eta && show_file_size {
            format!("{}: [{{bar:40.cyan/blue}}] {{pos:>5.2}}% | ETA: {{msg}}", description)
        } else if show_eta {
            format!("{}: [{{bar:40.cyan/blue}}] {{pos:>5.2}}% | {{msg}}", description)
        } else {
            format!("{}: [{{bar:40.cyan/blue}}] {{pos:>5.2}}%", description)
        };
        
        bar.set_style(
            ProgressStyle::default_bar()
                .template(&template)
                .unwrap()
                .progress_chars("█▉▊▋▌▍▎▏ "), // More detailed progress chars
        );
        
        let metrics = ProgressMetrics::new(total_duration, fps);
        let now = Instant::now();
        
        Self {
            bar,
            metrics,
            start_time: now,
            last_update: now,
            last_frame: 0,
            last_time: 0.0,
            stall_detection_enabled,
            stall_threshold: Duration::from_secs(stall_threshold_seconds),
            update_interval: Duration::from_millis(update_interval_ms),
            show_eta,
            show_file_size,
        }
    }
    
    pub fn update_from_ffmpeg_line(&mut self, line: &str) -> Result<bool> {
        let updated = self.metrics.update_from_ffmpeg_output(line);
        
        if updated {
            let now = Instant::now();
            
            // Throttle updates based on configured interval
            if now.duration_since(self.last_update) >= self.update_interval {
                self.update_display(now)?;
                self.last_update = now;
                return Ok(true);
            }
        }
        
        Ok(false)
    }
    
    fn update_display(&mut self, now: Instant) -> Result<()> {
        // Update progress bar position (multiply by 100 for precision)
        let position = (self.metrics.progress_percentage * 100.0) as u64;
        self.bar.set_position(position);
        
        // Check for stalls
        if self.stall_detection_enabled {
            self.check_for_stall(now)?;
        }
        
        // Build status message
        if self.show_eta || self.show_file_size {
            let message = self.build_status_message(now);
            self.bar.set_message(message);
        }
        
        // Update tracking variables
        self.last_frame = self.metrics.current_frame;
        self.last_time = self.metrics.current_time;
        
        Ok(())
    }
    
    fn check_for_stall(&mut self, now: Instant) -> Result<()> {
        let time_since_last = now.duration_since(self.last_update);
        
        // Check if progress has stalled
        if time_since_last >= self.stall_threshold {
            let frame_progress = self.metrics.current_frame > self.last_frame;
            let time_progress = self.metrics.current_time > self.last_time;
            
            if !frame_progress && !time_progress {
                warn!("Encoding appears to have stalled - no progress for {:.1} seconds", 
                      time_since_last.as_secs_f32());
                
                // Could implement stall recovery here
                debug!("Stall details - Current: frame={}, time={:.2}s | Last: frame={}, time={:.2}s",
                       self.metrics.current_frame, self.metrics.current_time,
                       self.last_frame, self.last_time);
            }
        }
        
        Ok(())
    }
    
    fn build_status_message(&self, now: Instant) -> String {
        let mut parts = Vec::new();
        
        // Add ETA calculation
        if self.show_eta && self.metrics.progress_percentage > 0.1 {
            let elapsed = now.duration_since(self.start_time);
            let estimated_total = Duration::from_secs_f64(
                elapsed.as_secs_f64() * (100.0 / self.metrics.progress_percentage as f64)
            );
            let eta = estimated_total.saturating_sub(elapsed);
            
            parts.push(format!("{:02}:{:02}", eta.as_secs() / 60, eta.as_secs() % 60));
        }
        
        // Add file size estimation
        if self.show_file_size {
            if let Some(estimated_size) = self.metrics.estimated_final_size {
                let size_mb = estimated_size as f64 / 1024.0;
                if size_mb < 1024.0 {
                    parts.push(format!("{:.1}MB", size_mb));
                } else {
                    parts.push(format!("{:.2}GB", size_mb / 1024.0));
                }
            } else if let Some(current_size) = self.metrics.size_kb {
                let size_mb = current_size as f64 / 1024.0;
                parts.push(format!("~{:.1}MB", size_mb));
            }
        }
        
        // Add encoding speed and FPS info
        if let Some(speed) = self.metrics.speed {
            parts.push(format!("{:.1}x", speed));
        }
        
        if let Some(fps) = self.metrics.fps {
            parts.push(format!("{:.1}fps", fps));
        }
        
        parts.join(" | ")
    }
    
    pub fn force_update(&mut self) -> Result<()> {
        let now = Instant::now();
        self.update_display(now)
    }
    
    pub fn get_metrics(&self) -> &ProgressMetrics {
        &self.metrics
    }
    
    pub fn finish_successfully(&self) {
        let elapsed = self.start_time.elapsed();
        let final_message = format!("Completed in {:02}:{:02}", 
                                   elapsed.as_secs() / 60, 
                                   elapsed.as_secs() % 60);
        
        self.bar.set_position(10000); // 100.00%
        self.bar.finish_with_message(final_message);
        
        info!("Encoding completed successfully in {:.1} seconds", elapsed.as_secs_f32());
    }
    
    pub fn finish_with_error(&self, error: &str) {
        let elapsed = self.start_time.elapsed();
        let final_message = format!("Failed after {:02}:{:02} - {}", 
                                   elapsed.as_secs() / 60, 
                                   elapsed.as_secs() % 60,
                                   error);
        
        self.bar.abandon_with_message(final_message);
    }
    
    pub fn is_stalled(&self) -> bool {
        if !self.stall_detection_enabled {
            return false;
        }
        
        let now = Instant::now();
        let time_since_last = now.duration_since(self.last_update);
        
        time_since_last >= self.stall_threshold && 
        self.metrics.current_frame == self.last_frame &&
        self.metrics.current_time == self.last_time
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_progress_metrics_creation() {
        let metrics = ProgressMetrics::new(3600.0, 24.0); // 1 hour at 24fps
        assert_eq!(metrics.total_frames, 86400);
        assert_eq!(metrics.total_duration, 3600.0);
    }

    #[test]
    fn test_ffmpeg_output_parsing() {
        let mut metrics = ProgressMetrics::new(100.0, 30.0);
        
        // Test frame parsing
        let updated = metrics.update_from_ffmpeg_output("frame= 1500 fps= 28 q=18.0 size=   12345kB time=00:00:50.00 bitrate=2024.3kbits/s speed=0.93x");
        assert!(updated);
        assert_eq!(metrics.current_frame, 1500);
        assert_eq!(metrics.current_time, 50.0);
        assert_eq!(metrics.speed, Some(0.93));
        assert!(metrics.progress_percentage > 0.0);
    }

    #[test] 
    fn test_stall_detection() {
        let tracker = EnhancedProgressTracker::new(
            3600.0, 30.0, "Test", true, 15, 1000, true, true
        );
        
        // Should not be stalled initially
        assert!(!tracker.is_stalled());
    }
}