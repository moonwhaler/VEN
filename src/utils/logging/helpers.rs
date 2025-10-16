//! Simple logging helper functions for common operations

/// Logs the start of an encoding operation
pub fn log_encoding_start(input: &str, output: &str, profile: &str, mode: &str) {
    tracing::info!(
        "Starting encoding: {} -> {} (profile: {}, mode: {})",
        input,
        output,
        profile,
        mode
    );
}

/// Logs the completion of an encoding operation
pub fn log_encoding_complete(duration: std::time::Duration, output_size: u64) {
    tracing::info!(
        "Encoding completed in {:.2}s, output size: {:.2} MB",
        duration.as_secs_f64(),
        output_size as f64 / 1_048_576.0
    );
}

/// Logs the result of content analysis
pub fn log_analysis_result(content_type: &str, grain_level: u8) {
    tracing::info!("Analysis: type={}, grain={}", content_type, grain_level);
}

/// Logs crop detection results
pub fn log_crop_detection(crop_values: &str) {
    tracing::info!("Crop detected: {}", crop_values);
}

/// Logs profile selection reasoning
pub fn log_profile_selection(profile: &str, reason: &str) {
    tracing::info!("Profile selected: {} ({})", profile, reason);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    // Note: These tests just ensure the functions compile and run without panicking
    // In a real scenario, you'd want to capture tracing output and verify it

    #[test]
    fn test_log_encoding_start() {
        log_encoding_start("input.mp4", "output.mp4", "default", "crf");
    }

    #[test]
    fn test_log_encoding_complete() {
        log_encoding_complete(Duration::from_secs(120), 104_857_600);
    }

    #[test]
    fn test_log_analysis_result() {
        log_analysis_result("anime", 3);
    }

    #[test]
    fn test_log_crop_detection() {
        log_crop_detection("1920:800:0:140");
    }

    #[test]
    fn test_log_profile_selection() {
        log_profile_selection("anime", "high grain detected");
    }
}
