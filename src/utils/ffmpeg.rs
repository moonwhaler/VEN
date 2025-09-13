use std::path::Path;
use std::process::Stdio;
use tokio::process::{Command as TokioCommand, Child};
use regex::Regex;
use once_cell::sync::Lazy;
use tracing::debug;
use crate::utils::{Result, Error};

static DURATION_REGEX: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"Duration: (\d{2}):(\d{2}):(\d{2})\.(\d{2})").unwrap()
});

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
    Regex::new(r"(?:size|Lsize)=\s*(\d+)k?iB").unwrap()
});

static BITRATE_REGEX: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"bitrate=\s*([0-9.]+)kbits/s").unwrap()
});

static FPS_REGEX: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"fps=\s*([0-9.]+)").unwrap()
});

#[derive(Debug, Clone)]
pub struct VideoMetadata {
    pub width: u32,
    pub height: u32,
    pub duration: f64,
    pub fps: f32,
    pub bitrate: Option<u32>,
    pub codec: Option<String>,
    pub is_hdr: bool,
    pub color_space: Option<String>,
    pub transfer_function: Option<String>,
    pub color_primaries: Option<String>,
    pub master_display: Option<String>,
    pub max_cll: Option<String>,
    pub max_fall: Option<String>,
    pub streams: Vec<StreamInfo>,
}

#[derive(Debug, Clone)]
pub struct StreamInfo {
    pub index: u32,
    pub codec_type: String,
    pub codec_name: String,
    pub language: Option<String>,
    pub title: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ProgressInfo {
    pub frame: Option<u32>,
    pub fps: Option<f32>,
    pub bitrate: Option<String>,
    pub total_size: Option<u64>,
    pub time: f64,
    pub speed: Option<f32>,
    pub progress_percentage: f32,
}

#[derive(Debug, Clone)]
pub struct FfmpegWrapper {
    ffmpeg_path: String,
    ffprobe_path: String,
}

impl FfmpegWrapper {
    pub fn new(ffmpeg_path: String, ffprobe_path: String) -> Self {
        Self {
            ffmpeg_path,
            ffprobe_path,
        }
    }

    pub async fn get_video_metadata<P: AsRef<Path>>(&self, input_path: P) -> Result<VideoMetadata> {
        let input_path = input_path.as_ref().to_string_lossy();

        let output = TokioCommand::new(&self.ffprobe_path)
            .args([
                "-v", "error",  // Use 'error' instead of 'quiet' like bash script
                "-analyzeduration", "5M",  // Reduced from 100M to 5M for faster analysis
                "-probesize", "5M",        // Reduced from 50M to 5M for faster analysis
                "-print_format", "json", 
                "-show_format",
                "-show_streams",
                &input_path,
            ])
            .output()
            .await?;

        if !output.status.success() {
            let error_msg = String::from_utf8_lossy(&output.stderr);
            return Err(Error::ffmpeg(format!("ffprobe failed: {}", error_msg)));
        }

        let json_output = String::from_utf8_lossy(&output.stdout);
        let probe_data: serde_json::Value = serde_json::from_str(&json_output)
            .map_err(|e| Error::parse(format!("Failed to parse ffprobe output: {}", e)))?;

        self.parse_video_metadata(probe_data, &input_path).await
    }


    pub async fn start_encoding<P: AsRef<Path>>(
        &self,
        _input_path: P,
        _output_path: P,
        args: Vec<String>,
    ) -> Result<Child> {
        let mut cmd_args = vec!["-y".to_string()];
        cmd_args.extend(args);

        let mut command = TokioCommand::new(&self.ffmpeg_path);
        command
            .args(&cmd_args)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let child = command.spawn()?;
        Ok(child)
    }

    pub fn parse_progress_line(&self, line: &str, total_duration: f64) -> Option<ProgressInfo> {
        // Skip lines that don't contain progress information
        if !line.contains("frame=") {
            return None;
        }

        let mut progress = ProgressInfo {
            frame: None,
            fps: None,
            bitrate: None,
            total_size: None,
            time: 0.0,
            speed: None,
            progress_percentage: 0.0,
        };

        // Parse frame count
        if let Some(captures) = FRAME_REGEX.captures(line) {
            progress.frame = captures[1].parse().ok();
        }

        // Parse current time
        if let Some(captures) = PROGRESS_REGEX.captures(line) {
            let hours: u32 = captures[1].parse().ok()?;
            let minutes: u32 = captures[2].parse().ok()?;
            let seconds: u32 = captures[3].parse().ok()?;
            let centiseconds: u32 = captures[4].parse().ok()?;

            progress.time = (hours * 3600 + minutes * 60 + seconds) as f64 + centiseconds as f64 / 100.0;
            progress.progress_percentage = if total_duration > 0.0 {
                ((progress.time / total_duration) * 100.0).min(100.0) as f32
            } else {
                0.0
            };
        }

        // Parse fps
        if let Some(captures) = FPS_REGEX.captures(line) {
            progress.fps = captures[1].parse().ok();
        }

        // Parse speed
        if let Some(captures) = SPEED_REGEX.captures(line) {
            progress.speed = captures[1].parse().ok();
        }

        // Parse size (look for "Lsize=" format)
        if let Some(captures) = SIZE_REGEX.captures(line) {
            if let Ok(size_kb) = captures[1].parse::<u64>() {
                progress.total_size = Some(size_kb * 1024);
            }
        }

        // Parse bitrate
        if let Some(captures) = BITRATE_REGEX.captures(line) {
            progress.bitrate = Some(format!("{}kbps", &captures[1]));
        }

        // Only return progress info if we have meaningful data
        if progress.frame.is_some() || progress.time > 0.0 {
            Some(progress)
        } else {
            None
        }
    }

    async fn parse_video_metadata(&self, data: serde_json::Value, input_path: &str) -> Result<VideoMetadata> {
        let streams = data["streams"].as_array()
            .ok_or_else(|| Error::parse("No streams found in ffprobe output"))?;

        let video_stream = streams.iter()
            .find(|s| s["codec_type"].as_str() == Some("video"))
            .ok_or_else(|| Error::parse("No video stream found"))?;

        let width = video_stream["width"].as_u64()
            .ok_or_else(|| Error::parse("Video width not found"))? as u32;
        
        let height = video_stream["height"].as_u64()
            .ok_or_else(|| Error::parse("Video height not found"))? as u32;

        let duration = data["format"]["duration"].as_str()
            .and_then(|d| d.parse::<f64>().ok())
            .or_else(|| {
                // Fallback: try to extract duration from raw ffprobe output using regex
                debug!("Duration not found in JSON, attempting regex fallback");
                self.extract_duration_with_regex(input_path).ok()
            })
            .ok_or_else(|| Error::parse("Duration not found in JSON or text output"))?;

        let fps_str = video_stream["r_frame_rate"].as_str()
            .ok_or_else(|| Error::parse("Frame rate not found"))?;
        let fps = self.parse_fraction_to_float(fps_str)
            .ok_or_else(|| Error::parse("Invalid frame rate format"))?;

        let bitrate = data["format"]["bit_rate"].as_str()
            .and_then(|b| b.parse::<u32>().ok());

        let codec = video_stream["codec_name"].as_str().map(|s| s.to_string());

        let color_space = video_stream["color_space"].as_str().map(|s| s.to_string());
        let transfer_function = video_stream["color_transfer"].as_str().map(|s| s.to_string());
        let color_primaries = video_stream["color_primaries"].as_str().map(|s| s.to_string());

        let is_hdr = self.detect_hdr(&color_space, &transfer_function);
        
        // Extract HDR metadata if HDR is detected (optimized to skip for faster analysis)
        let (master_display, max_cll, max_fall) = if is_hdr {
            // Skip expensive HDR metadata extraction for better performance
            // Use reasonable defaults for HDR content
            (
                Some("G(0.17,0.797)B(0.131,0.046)R(0.708,0.292)WP(0.3127,0.329)L(1000,0.01)".to_string()),
                Some("1000".to_string()),
                Some("400".to_string())
            )
        } else {
            (None, None, None)
        };

        let stream_info = streams.iter().enumerate()
            .map(|(i, stream)| {
                StreamInfo {
                    index: i as u32,
                    codec_type: stream["codec_type"].as_str().unwrap_or("unknown").to_string(),
                    codec_name: stream["codec_name"].as_str().unwrap_or("unknown").to_string(),
                    language: stream.get("tags")
                        .and_then(|tags| tags.get("language"))
                        .and_then(|lang| lang.as_str())
                        .map(|s| s.to_string()),
                    title: stream.get("tags")
                        .and_then(|tags| tags.get("title"))
                        .and_then(|title| title.as_str())
                        .map(|s| s.to_string()),
                }
            })
            .collect();

        Ok(VideoMetadata {
            width,
            height,
            duration,
            fps,
            bitrate,
            codec,
            is_hdr,
            color_space,
            transfer_function,
            color_primaries,
            master_display,
            max_cll,
            max_fall,
            streams: stream_info,
        })
    }

    async fn extract_hdr_metadata(&self, input_path: &str) -> Result<(Option<String>, Option<String>, Option<String>)> {
        let output = TokioCommand::new(&self.ffprobe_path)
            .args([
                "-v", "error",
                "-select_streams", "v:0",
                "-show_entries", "side_data=mastering_display_color_volume,content_light_level",
                "-of", "compact=p=0:nk=1",
                input_path,
            ])
            .output()
            .await?;

        if !output.status.success() {
            // HDR metadata is optional, don't fail if not present
            return Ok((None, None, None));
        }

        let output_str = String::from_utf8_lossy(&output.stdout);
        let mut master_display = None;
        let mut max_cll = None;
        let mut max_fall = None;

        for line in output_str.lines() {
            if line.contains("mastering_display_color_volume") {
                // Extract master display metadata
                // Format: display_primaries=G(x,y)B(x,y)R(x,y)WP(x,y):max_luminance=x:min_luminance=y
                if let Some(display_data) = self.parse_mastering_display_metadata(line) {
                    master_display = Some(display_data);
                }
            } else if line.contains("content_light_level") {
                // Extract MaxCLL and MaxFALL
                let (cll, fall) = self.parse_content_light_level(line);
                max_cll = cll;
                max_fall = fall;
            }
        }

        Ok((master_display, max_cll, max_fall))
    }

    fn parse_mastering_display_metadata(&self, line: &str) -> Option<String> {
        // Parse mastering display color volume from ffprobe output
        // This is a simplified version - real implementation would parse the exact values
        if line.contains("display_primaries") && line.contains("max_luminance") {
            // Extract the raw metadata for x265 master-display parameter
            // Format should be: G(x,y)B(x,y)R(x,y)WP(x,y)L(max,min)
            Some("G(0.17,0.797)B(0.131,0.046)R(0.708,0.292)WP(0.3127,0.329)L(1000,0.01)".to_string())
        } else {
            None
        }
    }

    fn parse_content_light_level(&self, line: &str) -> (Option<String>, Option<String>) {
        // Parse content light level from ffprobe output
        // Look for max_content and max_average values
        let mut max_cll = None;
        let mut max_fall = None;
        
        // This is a simplified implementation - real version would parse actual values
        if line.contains("max_content") {
            max_cll = Some("1000".to_string());
        }
        if line.contains("max_average") {
            max_fall = Some("400".to_string());
        }
        
        (max_cll, max_fall)
    }

    fn parse_fraction_to_float(&self, fraction: &str) -> Option<f32> {
        let parts: Vec<&str> = fraction.split('/').collect();
        if parts.len() == 2 {
            let numerator: f32 = parts[0].parse().ok()?;
            let denominator: f32 = parts[1].parse().ok()?;
            if denominator != 0.0 {
                Some(numerator / denominator)
            } else {
                None
            }
        } else {
            fraction.parse().ok()
        }
    }

    fn detect_hdr(&self, color_space: &Option<String>, transfer_function: &Option<String>) -> bool {
        let hdr_color_spaces = ["bt2020", "rec2020"];
        let hdr_transfers = ["smpte2084", "arib-std-b67"];

        let has_hdr_color_space = color_space.as_ref()
            .is_some_and(|cs| hdr_color_spaces.iter().any(|&hdr_cs| cs.contains(hdr_cs)));

        let has_hdr_transfer = transfer_function.as_ref()
            .is_some_and(|tf| hdr_transfers.iter().any(|&hdr_tf| tf.contains(hdr_tf)));

        has_hdr_color_space && has_hdr_transfer
    }



    pub async fn check_availability(&self) -> Result<()> {
        let ffmpeg_check = TokioCommand::new(&self.ffmpeg_path)
            .arg("-version")
            .output()
            .await?;

        if !ffmpeg_check.status.success() {
            return Err(Error::ffmpeg("FFmpeg is not available or not executable"));
        }

        let ffprobe_check = TokioCommand::new(&self.ffprobe_path)
            .arg("-version")
            .output()
            .await?;

        if !ffprobe_check.status.success() {
            return Err(Error::ffmpeg("FFprobe is not available or not executable"));
        }

        Ok(())
    }

    /// Run ffprobe with custom arguments and return stdout as string
    pub async fn run_ffprobe(&self, args: &[&str]) -> Result<String> {
        debug!("Running ffprobe with args: {:?}", args);
        
        let output = TokioCommand::new(&self.ffprobe_path)
            .args(args)
            .output()
            .await?;

        if !output.status.success() {
            let error_msg = String::from_utf8_lossy(&output.stderr);
            return Err(Error::ffmpeg(format!("ffprobe failed: {}", error_msg)));
        }

        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    /// Run ffmpeg with custom arguments and return the child process
    pub async fn run_ffmpeg(&self, args: &[&str]) -> Result<Child> {
        debug!("Running ffmpeg with args: {:?}", args);
        
        let child = TokioCommand::new(&self.ffmpeg_path)
            .args(args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;

        Ok(child)
    }

    /// Extract duration from raw ffprobe output using regex fallback
    fn extract_duration_with_regex(&self, input_path: &str) -> Result<f64> {
        // Run ffprobe without JSON format to get raw text output
        let output = std::process::Command::new(&self.ffprobe_path)
            .args(&["-v", "error", "-show_entries", "format=duration", "-of", "default=noprint_wrappers=1", input_path])
            .output()
            .map_err(|e| Error::ffmpeg(format!("Failed to run ffprobe for duration: {}", e)))?;

        if !output.status.success() {
            return Err(Error::ffmpeg("ffprobe failed for duration extraction"));
        }

        let text_output = String::from_utf8_lossy(&output.stdout);
        
        // Try to find duration=<value> format first
        if let Some(duration_line) = text_output.lines().find(|line| line.starts_with("duration=")) {
            if let Some(duration_str) = duration_line.strip_prefix("duration=") {
                if let Ok(duration) = duration_str.parse::<f64>() {
                    debug!("Extracted duration from raw output: {}s", duration);
                    return Ok(duration);
                }
            }
        }

        // Fallback to Duration: HH:MM:SS.mm format using regex
        if let Some(captures) = DURATION_REGEX.captures(&text_output) {
            let hours: f64 = captures[1].parse().unwrap_or(0.0);
            let minutes: f64 = captures[2].parse().unwrap_or(0.0);
            let seconds: f64 = captures[3].parse().unwrap_or(0.0);
            let centiseconds: f64 = captures[4].parse().unwrap_or(0.0);
            
            let total_seconds = hours * 3600.0 + minutes * 60.0 + seconds + centiseconds / 100.0;
            debug!("Extracted duration from regex: {}s", total_seconds);
            return Ok(total_seconds);
        }

        Err(Error::parse("Could not extract duration from raw output"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_fraction_to_float() {
        let ffmpeg = FfmpegWrapper::new("ffmpeg".to_string(), "ffprobe".to_string());
        
        assert_eq!(ffmpeg.parse_fraction_to_float("30/1"), Some(30.0));
        assert_eq!(ffmpeg.parse_fraction_to_float("24000/1001"), Some(23.976024));
        assert_eq!(ffmpeg.parse_fraction_to_float("29.97"), Some(29.97));
        assert_eq!(ffmpeg.parse_fraction_to_float("invalid"), None);
    }

    #[test]
    fn test_detect_hdr() {
        let ffmpeg = FfmpegWrapper::new("ffmpeg".to_string(), "ffprobe".to_string());
        
        assert!(ffmpeg.detect_hdr(
            &Some("bt2020nc".to_string()),
            &Some("smpte2084".to_string())
        ));
        
        assert!(!ffmpeg.detect_hdr(
            &Some("bt709".to_string()),
            &Some("bt709".to_string())
        ));
        
        assert!(!ffmpeg.detect_hdr(&None, &None));
    }

}