//! Encoding-related logging functionality

use chrono::Utc;
use std::io::Write;
use std::path::Path;

/// Logs encoding settings at the start of encoding
#[allow(clippy::too_many_arguments)]
pub fn log_encoding_settings<W: Write>(
    writer: &mut W,
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

    writeln!(writer, "FFmpeg Encoder - Encoding Log")?;
    writeln!(writer, "Generated: {}", timestamp)?;
    writeln!(writer, "========================================\n")?;

    writeln!(writer, "INPUT/OUTPUT:")?;
    writeln!(writer, "  Input:  {}", input_path.display())?;
    writeln!(writer, "  Output: {}", output_path.display())?;
    writeln!(writer)?;

    writeln!(writer, "ENCODING SETTINGS:")?;
    writeln!(writer, "  Mode: {}", mode.to_uppercase())?;
    writeln!(
        writer,
        "  Profile: {} - {}",
        profile_name, profile_settings.title
    )?;
    writeln!(
        writer,
        "  Content Type: {:?}",
        profile_settings.content_type
    )?;
    writeln!(writer, "  Adaptive CRF: {}", adaptive_crf)?;
    writeln!(writer, "  Adaptive Bitrate: {} kbps", adaptive_bitrate)?;
    writeln!(writer)?;

    if let Some(filters) = filter_chain {
        writeln!(writer, "VIDEO FILTERS:")?;
        writeln!(writer, "  {}", filters)?;
        writeln!(writer)?;
    }

    writeln!(writer, "STREAM MAPPING:")?;
    writeln!(writer, "  {}", stream_mapping)?;
    writeln!(writer)?;

    writeln!(writer, "x265 PARAMETERS:")?;
    for (key, value) in &profile_settings.x265_params {
        writeln!(writer, "  {}: {}", key, value)?;
    }
    writeln!(writer)?;

    writer.flush()?;
    Ok(())
}

/// Logs encoding progress messages
pub fn log_encoding_progress<W: Write>(writer: &mut W, message: &str) -> crate::utils::Result<()> {
    let timestamp = Utc::now().format("%H:%M:%S");
    writeln!(writer, "[{}] {}", timestamp, message)?;
    writer.flush()?;
    Ok(())
}

/// Logs encoding completion status
pub fn log_encoding_complete<W: Write>(
    writer: &mut W,
    success: bool,
    duration: std::time::Duration,
    output_size: Option<u64>,
    exit_code: Option<i32>,
) -> crate::utils::Result<()> {
    writeln!(writer, "ENCODING RESULT:")?;
    writeln!(
        writer,
        "  Status: {}",
        if success { "SUCCESS" } else { "FAILED" }
    )?;
    writeln!(writer, "  Duration: {:.2}s", duration.as_secs_f64())?;

    if let Some(size) = output_size {
        writeln!(writer, "  Output Size: {:.2} MB", size as f64 / 1_048_576.0)?;
    }

    if let Some(code) = exit_code {
        writeln!(writer, "  Exit Code: {}", code)?;
    }

    let timestamp = Utc::now().format("%Y-%m-%d %H:%M:%S UTC");
    writeln!(writer, "  Completed: {}", timestamp)?;
    writeln!(writer)?;

    writer.flush()?;
    Ok(())
}

/// Logs the raw FFmpeg command
pub fn log_ffmpeg_command<W: Write>(
    writer: &mut W,
    ffmpeg_path: &str,
    args: &[String],
) -> crate::utils::Result<()> {
    writeln!(writer, "RAW FFMPEG COMMAND:")?;

    // Build the complete command exactly as it will be executed
    // start_encoding() adds -y at the beginning, so we replicate that here
    let mut full_command = vec![ffmpeg_path.to_string(), "-y".to_string()];
    full_command.extend_from_slice(args);

    // Write as a single line that can be copy-pasted
    writeln!(writer, "  {}", full_command.join(" "))?;
    writeln!(writer)?;

    writer.flush()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_log_encoding_progress() {
        let mut buffer = Vec::new();
        let result = log_encoding_progress(&mut buffer, "Test progress message");
        assert!(result.is_ok());
        let output = String::from_utf8(buffer).unwrap();
        assert!(output.contains("Test progress message"));
    }

    #[test]
    fn test_log_encoding_complete_success() {
        let mut buffer = Vec::new();
        let result = log_encoding_complete(
            &mut buffer,
            true,
            Duration::from_secs(120),
            Some(104_857_600),
            Some(0),
        );
        assert!(result.is_ok());
        let output = String::from_utf8(buffer).unwrap();
        assert!(output.contains("SUCCESS"));
        assert!(output.contains("120.00s"));
        assert!(output.contains("100.00 MB"));
        assert!(output.contains("Exit Code: 0"));
    }

    #[test]
    fn test_log_encoding_complete_failure() {
        let mut buffer = Vec::new();
        let result = log_encoding_complete(
            &mut buffer,
            false,
            Duration::from_secs(30),
            None,
            Some(1),
        );
        assert!(result.is_ok());
        let output = String::from_utf8(buffer).unwrap();
        assert!(output.contains("FAILED"));
        assert!(output.contains("Exit Code: 1"));
    }

    #[test]
    fn test_log_ffmpeg_command() {
        let mut buffer = Vec::new();
        let args = vec![
            "-i".to_string(),
            "input.mp4".to_string(),
            "-c:v".to_string(),
            "libx265".to_string(),
            "output.mp4".to_string(),
        ];
        let result = log_ffmpeg_command(&mut buffer, "/usr/bin/ffmpeg", &args);
        assert!(result.is_ok());
        let output = String::from_utf8(buffer).unwrap();
        assert!(output.contains("RAW FFMPEG COMMAND"));
        assert!(output.contains("/usr/bin/ffmpeg -y -i input.mp4"));
    }
}
