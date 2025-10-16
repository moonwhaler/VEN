//! Crop detection logging functionality

use std::io::Write;

/// Logs crop detection results to the file
#[allow(clippy::too_many_arguments)]
pub fn log_crop_detection_results<W: Write>(
    writer: &mut W,
    enabled: bool,
    sample_count: u32,
    sample_timestamps: &[f64],
    crop_result: Option<&str>,
    detection_method: &str,
    sdr_limit: u32,
    hdr_limit: u32,
    is_hdr: bool,
) -> crate::utils::Result<()> {
    writeln!(writer, "CROP DETECTION:")?;
    writeln!(writer, "  Enabled: {}", if enabled { "Yes" } else { "No" })?;

    if !enabled {
        writeln!(writer)?;
        writer.flush()?;
        return Ok(());
    }

    writeln!(writer, "  Sample Count: {}", sample_count)?;

    // Format timestamps for display
    let timestamp_display =
        if sample_timestamps.len() == 1 && (sample_timestamps[0] + 1.0).abs() < f64::EPSILON {
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
    writeln!(
        writer,
        "  Crop Threshold: {} ({} content)",
        used_limit,
        if is_hdr { "HDR" } else { "SDR" }
    )?;

    match crop_result {
        Some(crop) => {
            writeln!(writer, "  Result: CROP DETECTED")?;
            writeln!(writer, "  Crop Values: {}", crop)?;

            // Parse and calculate crop statistics
            if let Some(stats) = parse_crop_statistics(crop) {
                writeln!(writer, "  Original Resolution: {}x{}", stats.0, stats.1)?;
                writeln!(writer, "  Cropped Resolution: {}x{}", stats.2, stats.3)?;
                writeln!(writer, "  Pixels Removed: {:.1}%", stats.4)?;
            }
        }
        None => {
            writeln!(writer, "  Result: NO CROP DETECTED")?;
            writeln!(
                writer,
                "  Reason: No consistent black bars found across sample points"
            )?;
        }
    }

    writeln!(writer)?;
    writer.flush()?;
    Ok(())
}

/// Parses crop statistics from a crop string
/// Returns (original_width, original_height, cropped_width, cropped_height, percent_removed)
fn parse_crop_statistics(crop_str: &str) -> Option<(u32, u32, u32, u32, f32)> {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_crop_statistics_1080p() {
        let result = parse_crop_statistics("1920:800:0:140");
        assert!(result.is_some());
        let (orig_w, orig_h, crop_w, crop_h, percent) = result.unwrap();
        assert_eq!(orig_w, 1920);
        assert_eq!(orig_h, 1080);
        assert_eq!(crop_w, 1920);
        assert_eq!(crop_h, 800);
        assert!(percent > 0.0 && percent < 100.0);
    }

    #[test]
    fn test_parse_crop_statistics_4k() {
        let result = parse_crop_statistics("3840:1600:0:280");
        assert!(result.is_some());
        let (orig_w, orig_h, crop_w, crop_h, _) = result.unwrap();
        assert_eq!(orig_w, 3840);
        assert_eq!(orig_h, 2160);
        assert_eq!(crop_w, 3840);
        assert_eq!(crop_h, 1600);
    }

    #[test]
    fn test_parse_crop_statistics_invalid() {
        assert!(parse_crop_statistics("invalid").is_none());
        assert!(parse_crop_statistics("1920:800").is_none());
        assert!(parse_crop_statistics("abc:def:ghi:jkl").is_none());
    }

    #[test]
    fn test_log_crop_detection_disabled() {
        let mut buffer = Vec::new();
        let result = log_crop_detection_results(
            &mut buffer,
            false,
            0,
            &[],
            None,
            "smart",
            24,
            32,
            false,
        );
        assert!(result.is_ok());
        let output = String::from_utf8(buffer).unwrap();
        assert!(output.contains("Enabled: No"));
    }

    #[test]
    fn test_log_crop_detection_no_crop() {
        let mut buffer = Vec::new();
        let result = log_crop_detection_results(
            &mut buffer,
            true,
            5,
            &[10.0, 20.0, 30.0, 40.0, 50.0],
            None,
            "smart",
            24,
            32,
            false,
        );
        assert!(result.is_ok());
        let output = String::from_utf8(buffer).unwrap();
        assert!(output.contains("NO CROP DETECTED"));
        assert!(output.contains("10.0s, 20.0s"));
    }

    #[test]
    fn test_log_crop_detection_with_crop() {
        let mut buffer = Vec::new();
        let result = log_crop_detection_results(
            &mut buffer,
            true,
            5,
            &[10.0, 20.0, 30.0, 40.0, 50.0],
            Some("1920:800:0:140"),
            "smart",
            24,
            32,
            false,
        );
        assert!(result.is_ok());
        let output = String::from_utf8(buffer).unwrap();
        assert!(output.contains("CROP DETECTED"));
        assert!(output.contains("1920:800:0:140"));
        assert!(output.contains("Original Resolution"));
    }
}
