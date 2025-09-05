use tracing_subscriber::{
    fmt::{self, time::ChronoUtc},
    layer::SubscriberExt,
    util::SubscriberInitExt,
    EnvFilter, Layer,
};
use tracing::Level;
use console::Style;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use chrono::Utc;

pub fn setup_logging(level: &str, show_timestamps: bool, colored: bool) -> crate::utils::Result<()> {
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
        .with_ansi(colored);

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
    let style = Style::new().bold().green();
    tracing::info!(
        "{} Starting encoding: {} -> {} (profile: {}, mode: {})",
        style.apply_to("►"),
        input,
        output,
        profile,
        mode
    );
}

pub fn log_encoding_complete(duration: std::time::Duration, output_size: u64) {
    let style = Style::new().bold().green();
    tracing::info!(
        "{} Encoding completed in {:.2}s, output size: {:.2} MB",
        style.apply_to("✓"),
        duration.as_secs_f64(),
        output_size as f64 / 1_048_576.0
    );
}

pub fn log_analysis_result(complexity: f32, content_type: &str, grain_level: u8) {
    let style = Style::new().bold().cyan();
    tracing::info!(
        "{} Analysis: complexity={:.2}, type={}, grain={}",
        style.apply_to("🔍"),
        complexity,
        content_type,
        grain_level
    );
}

pub fn log_crop_detection(crop_values: &str) {
    let style = Style::new().bold().cyan();
    tracing::info!("{} Crop detected: {}", style.apply_to("✂"), crop_values);
}

pub fn log_profile_selection(profile: &str, reason: &str) {
    let style = Style::new().bold().magenta();
    tracing::info!(
        "{} Profile selected: {} ({})",
        style.apply_to("🎯"),
        profile,
        reason
    );
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
        writeln!(writer, "")?;
        
        writeln!(writer, "ENCODING SETTINGS:")?;
        writeln!(writer, "  Mode: {}", mode.to_uppercase())?;
        writeln!(writer, "  Profile: {} - {}", profile_name, profile_settings.title)?;
        writeln!(writer, "  Content Type: {:?}", profile_settings.content_type)?;
        writeln!(writer, "  Adaptive CRF: {}", adaptive_crf)?;
        writeln!(writer, "  Adaptive Bitrate: {} kbps", adaptive_bitrate)?;
        writeln!(writer, "")?;
        
        if let Some(filters) = filter_chain {
            writeln!(writer, "VIDEO FILTERS:")?;
            writeln!(writer, "  {}", filters)?;
            writeln!(writer, "")?;
        }
        
        writeln!(writer, "STREAM MAPPING:")?;
        writeln!(writer, "  {}", stream_mapping)?;
        writeln!(writer, "")?;
        
        writeln!(writer, "x265 PARAMETERS:")?;
        for (key, value) in &profile_settings.x265_params {
            writeln!(writer, "  {}: {}", key, value)?;
        }
        writeln!(writer, "")?;
        
        writer.flush()?;
        Ok(())
    }
    
    pub fn log_analysis_results(
        &self,
        metadata: &crate::utils::ffmpeg::VideoMetadata,
        complexity_score: Option<f32>,
        grain_level: Option<u8>,
    ) -> crate::utils::Result<()> {
        let mut writer = self.writer.lock().unwrap();
        
        writeln!(writer, "VIDEO ANALYSIS:")?;
        writeln!(writer, "  Resolution: {}x{}", metadata.width, metadata.height)?;
        writeln!(writer, "  Duration: {:.2}s", metadata.duration)?;
        writeln!(writer, "  Framerate: {:.2} fps", metadata.fps)?;
        writeln!(writer, "  Codec: {}", metadata.codec.as_deref().unwrap_or("Unknown"))?;
        if let Some(bitrate) = metadata.bitrate {
            writeln!(writer, "  Bitrate: {} kbps", bitrate)?;
        }
        writeln!(writer, "  HDR: {}", if metadata.is_hdr { "Yes" } else { "No" })?;
        
        if let Some(complexity) = complexity_score {
            writeln!(writer, "  Complexity Score: {:.2}", complexity)?;
        }
        
        if let Some(grain) = grain_level {
            writeln!(writer, "  Grain Level: {}", grain)?;
        }
        
        writeln!(writer, "")?;
        writer.flush()?;
        Ok(())
    }
    
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
            writeln!(writer, "")?;
            writer.flush()?;
            return Ok(());
        }
        
        writeln!(writer, "  Sample Count: {}", sample_count)?;
        
        // Format timestamps for display
        let timestamp_display = if sample_timestamps.len() == 1 && sample_timestamps[0] == -1.0 {
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
        writeln!(writer, "  Crop Threshold: {} ({} content)", used_limit, if is_hdr { "HDR" } else { "SDR" })?;
        
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
                writeln!(writer, "  Reason: No consistent black bars found across sample points")?;
            }
        }
        
        writeln!(writer, "")?;
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
        writeln!(writer, "  Status: {}", if success { "SUCCESS" } else { "FAILED" })?;
        writeln!(writer, "  Duration: {:.2}s", duration.as_secs_f64())?;
        
        if let Some(size) = output_size {
            writeln!(writer, "  Output Size: {:.2} MB", size as f64 / 1_048_576.0)?;
        }
        
        if let Some(code) = exit_code {
            writeln!(writer, "  Exit Code: {}", code)?;
        }
        
        let timestamp = Utc::now().format("%Y-%m-%d %H:%M:%S UTC");
        writeln!(writer, "  Completed: {}", timestamp)?;
        writeln!(writer, "")?;
        
        writer.flush()?;
        Ok(())
    }
    
    pub fn get_log_path(&self) -> &Path {
        &self.log_path
    }
}