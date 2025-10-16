//! Message filtering to remove noisy log output

/// Checks if a message should be shown based on noise patterns
/// Returns true if the message should be displayed, false if it should be filtered out
pub fn should_show_message(message: &str) -> bool {
    // Filter out noisy FFmpeg messages that don't add value
    let noise_patterns = [
        "Invalid Block Addition value",
        "Could not find codec parameters for stream",
        "Consider increasing the value for the 'analyzeduration'",
        "x265 [info]: HEVC encoder version",
        "x265 [info]: build info",
        "x265 [info]: using cpu capabilities",
        "x265 [info]: Thread pool created",
        "x265 [info]: Slices",
        "x265 [info]: frame threads",
        "x265 [info]: Coding QT",
        "x265 [info]: Residual QT",
        "x265 [info]: ME / range",
        "x265 [info]: Keyframe min",
        "x265 [info]: Lookahead",
        "x265 [info]: b-pyramid",
        "x265 [info]: References",
        "x265 [info]: AQ:",
        "x265 [info]: Rate Control",
        "x265 [info]: tools:",
        "matroska,webm",
    ];

    !noise_patterns
        .iter()
        .any(|pattern| message.contains(pattern))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_should_show_normal_message() {
        assert!(should_show_message("Processing file: test.mp4"));
        assert!(should_show_message("Encoding completed successfully"));
    }

    #[test]
    fn test_should_filter_x265_info() {
        assert!(!should_show_message("x265 [info]: HEVC encoder version 3.5"));
        assert!(!should_show_message("x265 [info]: build info"));
        assert!(!should_show_message("x265 [info]: using cpu capabilities: MMX2 SSE2"));
    }

    #[test]
    fn test_should_filter_ffmpeg_noise() {
        assert!(!should_show_message("Invalid Block Addition value"));
        assert!(!should_show_message(
            "Could not find codec parameters for stream 2"
        ));
    }

    #[test]
    fn test_should_show_x265_non_info() {
        assert!(should_show_message("x265 [warning]: Something went wrong"));
        assert!(should_show_message("x265 [error]: Failed to encode"));
    }
}
