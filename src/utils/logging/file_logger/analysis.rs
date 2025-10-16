//! Video analysis and content analysis logging functionality

use std::io::Write;

/// Logs comprehensive video analysis results including HDR/DV information
pub fn log_analysis_results<W: Write>(
    writer: &mut W,
    metadata: &crate::utils::ffmpeg::VideoMetadata,
    grain_level: Option<u8>,
    content_analysis: Option<&crate::content_manager::ContentAnalysisResult>,
) -> crate::utils::Result<()> {
    writeln!(writer, "VIDEO ANALYSIS:")?;
    writeln!(
        writer,
        "  Resolution: {}x{}",
        metadata.width, metadata.height
    )?;
    writeln!(writer, "  Duration: {:.2}s", metadata.duration)?;
    writeln!(writer, "  Framerate: {:.2} fps", metadata.fps)?;
    writeln!(
        writer,
        "  Codec: {}",
        metadata.codec.as_deref().unwrap_or("Unknown")
    )?;
    if let Some(bitrate) = metadata.bitrate {
        writeln!(writer, "  Bitrate: {} kbps", bitrate)?;
    }
    writeln!(
        writer,
        "  HDR: {}",
        if metadata.is_hdr { "Yes" } else { "No" }
    )?;

    if let Some(grain) = grain_level {
        writeln!(writer, "  Grain Level: {}", grain)?;
    }

    // Enhanced HDR/DV analysis logging
    if let Some(analysis) = content_analysis {
        writeln!(writer)?;
        writeln!(writer, "HDR/DOLBY VISION ANALYSIS:")?;

        match &analysis.recommended_approach {
            crate::content_manager::ContentEncodingApproach::SDR => {
                writeln!(writer, "  Content Type: SDR (Standard Dynamic Range)")?;
                writeln!(writer, "  HDR Format: None")?;
            }
            crate::content_manager::ContentEncodingApproach::HDR(hdr_result) => {
                log_hdr_info(writer, hdr_result)?;
            }
            crate::content_manager::ContentEncodingApproach::DolbyVision(dv_info) => {
                log_dolby_vision_info(writer, dv_info)?;
            }
            crate::content_manager::ContentEncodingApproach::DolbyVisionWithHDR10Plus(
                dv_info,
                hdr_result,
            ) => {
                log_dual_format_info(writer, dv_info, hdr_result)?;
            }
        }

        // Encoding adjustments section
        log_encoding_adjustments(writer, &analysis.encoding_adjustments)?;

        // HDR10+ specific information
        if let Some(ref hdr10plus_result) = analysis.hdr10_plus {
            log_hdr10plus_metadata(writer, hdr10plus_result)?;
        }
    }

    writeln!(writer)?;
    writer.flush()?;
    Ok(())
}

/// Logs HDR information
fn log_hdr_info<W: Write>(
    writer: &mut W,
    hdr_result: &crate::hdr::HdrAnalysisResult,
) -> crate::utils::Result<()> {
    writeln!(writer, "  Content Type: HDR (High Dynamic Range)")?;
    writeln!(writer, "  HDR Format: {:?}", hdr_result.metadata.format)?;
    writeln!(
        writer,
        "  Detection Confidence: {:.1}%",
        hdr_result.confidence_score * 100.0
    )?;

    // Color space information
    if let Some(ref cs) = hdr_result.metadata.raw_color_space {
        writeln!(writer, "  Color Space: {}", cs)?;
    }
    if let Some(ref tf) = hdr_result.metadata.raw_transfer {
        writeln!(writer, "  Transfer Function: {}", tf)?;
    }
    if let Some(ref cp) = hdr_result.metadata.raw_primaries {
        writeln!(writer, "  Color Primaries: {}", cp)?;
    }

    // Mastering display metadata
    if let Some(ref master_display) = hdr_result.metadata.master_display {
        writeln!(writer, "  Mastering Display Metadata:")?;
        writeln!(
            writer,
            "    Red Primary: ({:.4}, {:.4})",
            master_display.red_primary.0, master_display.red_primary.1
        )?;
        writeln!(
            writer,
            "    Green Primary: ({:.4}, {:.4})",
            master_display.green_primary.0, master_display.green_primary.1
        )?;
        writeln!(
            writer,
            "    Blue Primary: ({:.4}, {:.4})",
            master_display.blue_primary.0, master_display.blue_primary.1
        )?;
        writeln!(
            writer,
            "    White Point: ({:.4}, {:.4})",
            master_display.white_point.0, master_display.white_point.1
        )?;
        writeln!(
            writer,
            "    Max Luminance: {} nits",
            master_display.max_luminance
        )?;
        writeln!(
            writer,
            "    Min Luminance: {:.4} nits",
            master_display.min_luminance
        )?;
    }

    // Content light level information
    if let Some(ref cll) = hdr_result.metadata.content_light_level {
        writeln!(writer, "  Content Light Level:")?;
        writeln!(writer, "    Max CLL: {} nits", cll.max_cll)?;
        writeln!(writer, "    Max FALL: {} nits", cll.max_fall)?;
    }

    Ok(())
}

/// Logs Dolby Vision information
fn log_dolby_vision_info<W: Write>(
    writer: &mut W,
    dv_info: &crate::analysis::dolby_vision::DolbyVisionInfo,
) -> crate::utils::Result<()> {
    writeln!(writer, "  Content Type: Dolby Vision")?;
    writeln!(
        writer,
        "  Dolby Vision Profile: {}",
        dv_info.profile.as_str()
    )?;
    writeln!(
        writer,
        "  Profile Description: {}",
        get_dv_profile_description(&dv_info.profile)
    )?;
    writeln!(
        writer,
        "  RPU Present: {}",
        if dv_info.rpu_present { "Yes" } else { "No" }
    )?;
    writeln!(
        writer,
        "  Has Enhancement Layer: {}",
        if dv_info.has_enhancement_layer {
            "Yes"
        } else {
            "No"
        }
    )?;
    writeln!(
        writer,
        "  EL Present: {}",
        if dv_info.el_present { "Yes" } else { "No" }
    )?;
    writeln!(
        writer,
        "  HDR10 Compatible: {}",
        if dv_info.profile.supports_hdr10_compatibility() {
            "Yes"
        } else {
            "No"
        }
    )?;
    writeln!(
        writer,
        "  Dual Layer: {}",
        if dv_info.profile.is_dual_layer() {
            "Yes"
        } else {
            "No"
        }
    )?;

    if let Some(bl_compatible_id) = dv_info.bl_compatible_id {
        writeln!(writer, "  BL Compatible ID: {}", bl_compatible_id)?;
    }

    if let Some(ref codec_profile) = dv_info.codec_profile {
        writeln!(writer, "  Codec Profile: {}", codec_profile)?;
    }

    Ok(())
}

/// Logs dual format (Dolby Vision + HDR10+) information
fn log_dual_format_info<W: Write>(
    writer: &mut W,
    dv_info: &crate::analysis::dolby_vision::DolbyVisionInfo,
    hdr_result: &crate::hdr::HdrAnalysisResult,
) -> crate::utils::Result<()> {
    writeln!(
        writer,
        "  Content Type: Dual Format (Dolby Vision + HDR10+)"
    )?;

    // Dolby Vision information
    writeln!(writer, "  Dolby Vision:")?;
    writeln!(writer, "    Profile: {}", dv_info.profile.as_str())?;
    writeln!(
        writer,
        "    Profile Description: {}",
        get_dv_profile_description(&dv_info.profile)
    )?;
    writeln!(
        writer,
        "    RPU Present: {}",
        if dv_info.rpu_present { "Yes" } else { "No" }
    )?;
    writeln!(
        writer,
        "    HDR10 Compatible: {}",
        if dv_info.profile.supports_hdr10_compatibility() {
            "Yes"
        } else {
            "No"
        }
    )?;
    writeln!(
        writer,
        "    EL Present: {}",
        if dv_info.el_present { "Yes" } else { "No" }
    )?;

    // HDR10+ information
    writeln!(writer, "  HDR10+ Format: {:?}", hdr_result.metadata.format)?;
    writeln!(
        writer,
        "  HDR Detection Confidence: {:.1}%",
        hdr_result.confidence_score * 100.0
    )?;

    if let Some(ref master_display) = hdr_result.metadata.master_display {
        writeln!(
            writer,
            "  Max Luminance: {} nits",
            master_display.max_luminance
        )?;
        writeln!(
            writer,
            "  Min Luminance: {:.4} nits",
            master_display.min_luminance
        )?;
    }

    if let Some(ref cll) = hdr_result.metadata.content_light_level {
        writeln!(writer, "  Max CLL: {} nits", cll.max_cll)?;
        writeln!(writer, "  Max FALL: {} nits", cll.max_fall)?;
    }

    Ok(())
}

/// Logs encoding adjustments based on content analysis
fn log_encoding_adjustments<W: Write>(
    writer: &mut W,
    adjustments: &crate::content_manager::EncodingAdjustments,
) -> crate::utils::Result<()> {
    writeln!(writer)?;
    writeln!(writer, "CONTENT-BASED ENCODING ADJUSTMENTS:")?;
    writeln!(
        writer,
        "  CRF Adjustment: {:+.1}",
        adjustments.crf_adjustment
    )?;
    writeln!(
        writer,
        "  Bitrate Multiplier: {:.2}x",
        adjustments.bitrate_multiplier
    )?;
    writeln!(
        writer,
        "  Encoding Complexity: {:.2}x",
        adjustments.encoding_complexity
    )?;
    writeln!(
        writer,
        "  Recommended CRF Range: {:.1}-{:.1}",
        adjustments.recommended_crf_range.0, adjustments.recommended_crf_range.1
    )?;

    if adjustments.requires_vbv {
        writeln!(writer, "  VBV Required: Yes")?;
        if let Some(bufsize) = adjustments.vbv_bufsize {
            writeln!(writer, "  VBV Buffer Size: {} kbps", bufsize)?;
        }
        if let Some(maxrate) = adjustments.vbv_maxrate {
            writeln!(writer, "  VBV Max Rate: {} kbps", maxrate)?;
        }
    } else {
        writeln!(writer, "  VBV Required: No")?;
    }

    Ok(())
}

/// Logs HDR10+ dynamic metadata information
fn log_hdr10plus_metadata<W: Write>(
    writer: &mut W,
    hdr10plus_result: &crate::hdr10plus::Hdr10PlusProcessingResult,
) -> crate::utils::Result<()> {
    writeln!(writer)?;
    writeln!(writer, "HDR10+ DYNAMIC METADATA:")?;
    writeln!(
        writer,
        "  Extraction Successful: {}",
        if hdr10plus_result.extraction_successful {
            "Yes"
        } else {
            "No"
        }
    )?;
    writeln!(
        writer,
        "  Metadata File: {}",
        hdr10plus_result.metadata_file.display()
    )?;
    writeln!(writer, "  Curve Count: {}", hdr10plus_result.curve_count)?;
    writeln!(writer, "  Scene Count: {}", hdr10plus_result.scene_count)?;

    if let Some(file_size) = hdr10plus_result.file_size {
        writeln!(writer, "  Metadata File Size: {} bytes", file_size)?;
    }

    // Access metadata fields directly since it's not optional
    let metadata = &hdr10plus_result.metadata;
    writeln!(writer, "  Metadata Version: {}", metadata.version)?;
    writeln!(writer, "  Frame Count: {}", metadata.num_frames)?;

    if let Some(ref source) = metadata.source {
        writeln!(
            writer,
            "  Source: {}",
            source.filename.as_deref().unwrap_or("Unknown")
        )?;
        if let Some(resolution) = &source.resolution {
            writeln!(writer, "  Source Resolution: {}", resolution)?;
        }
        if let Some(frame_rate) = source.frame_rate {
            writeln!(writer, "  Source Frame Rate: {:.2} fps", frame_rate)?;
        }
    }

    // Scene information
    if let Some(ref scene_info) = metadata.scene_info {
        writeln!(writer, "  Scene Count: {}", scene_info.len())?;
        for (i, scene) in scene_info.iter().enumerate().take(3) {
            // Limit to first 3
            writeln!(
                writer,
                "    Scene {}: Frames {}-{}, Avg MaxRGB: {:.2}",
                i + 1,
                scene.first_frame,
                scene.last_frame,
                scene.average_maxrgb.unwrap_or(0.0)
            )?;
        }
        if scene_info.len() > 3 {
            writeln!(writer, "    ... and {} more scenes", scene_info.len() - 3)?;
        }
    }

    // Frame metadata summary
    if !metadata.frames.is_empty() {
        writeln!(
            writer,
            "  Frame Metadata: {} frames with tone mapping data",
            metadata.frames.len()
        )?;
        if let Some(first_frame) = metadata.frames.first() {
            if let Some(app_version) = first_frame.application_version {
                writeln!(writer, "  Application Version: {}", app_version)?;
            }
            if let Some(target_lum) = first_frame.targeted_system_display_maximum_luminance {
                writeln!(writer, "  Target Max Luminance: {:.1} nits", target_lum)?;
            }
        }
    }

    Ok(())
}

/// Helper method to get Dolby Vision profile descriptions
fn get_dv_profile_description(
    profile: &crate::analysis::dolby_vision::DolbyVisionProfile,
) -> &'static str {
    match profile {
        crate::analysis::dolby_vision::DolbyVisionProfile::None => "Not Dolby Vision",
        crate::analysis::dolby_vision::DolbyVisionProfile::Profile5 => "Single-layer DV only",
        crate::analysis::dolby_vision::DolbyVisionProfile::Profile7 => {
            "Dual-layer (BL + EL + RPU)"
        }
        crate::analysis::dolby_vision::DolbyVisionProfile::Profile81 => {
            "Single-layer with HDR10 compatibility"
        }
        crate::analysis::dolby_vision::DolbyVisionProfile::Profile82 => {
            "Single-layer with SDR compatibility"
        }
        crate::analysis::dolby_vision::DolbyVisionProfile::Profile84 => "HDMI streaming profile",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_dv_profile_description() {
        use crate::analysis::dolby_vision::DolbyVisionProfile;

        assert_eq!(
            get_dv_profile_description(&DolbyVisionProfile::None),
            "Not Dolby Vision"
        );
        assert_eq!(
            get_dv_profile_description(&DolbyVisionProfile::Profile5),
            "Single-layer DV only"
        );
        assert_eq!(
            get_dv_profile_description(&DolbyVisionProfile::Profile81),
            "Single-layer with HDR10 compatibility"
        );
    }
}
