use super::types::*;
use crate::utils::{Result, Error};

pub struct HdrMetadataExtractor;

impl HdrMetadataExtractor {
    /// Parse raw master display metadata string into structured data
    pub fn parse_master_display(raw: &str) -> Result<MasteringDisplayColorVolume> {
        // Parse format: "G(x,y)B(x,y)R(x,y)WP(x,y)L(max,min)"
        let re = regex::Regex::new(
            r"G\(([0-9.]+),([0-9.]+)\)B\(([0-9.]+),([0-9.]+)\)R\(([0-9.]+),([0-9.]+)\)WP\(([0-9.]+),([0-9.]+)\)L\(([0-9.]+),([0-9.]+)\)"
        ).map_err(|e| Error::parse(format!("Regex compilation failed: {}", e)))?;

        if let Some(captures) = re.captures(raw) {
            let parse_float = |i: usize| -> Result<f32> {
                captures.get(i)
                    .ok_or_else(|| Error::parse(format!("Missing capture group {}", i)))?
                    .as_str()
                    .parse()
                    .map_err(|e| Error::parse(format!("Failed to parse float: {}", e)))
            };

            let parse_u32 = |i: usize| -> Result<u32> {
                captures.get(i)
                    .ok_or_else(|| Error::parse(format!("Missing capture group {}", i)))?
                    .as_str()
                    .parse()
                    .map_err(|e| Error::parse(format!("Failed to parse u32: {}", e)))
            };

            Ok(MasteringDisplayColorVolume {
                green_primary: (parse_float(1)?, parse_float(2)?),
                blue_primary: (parse_float(3)?, parse_float(4)?),
                red_primary: (parse_float(5)?, parse_float(6)?),
                white_point: (parse_float(7)?, parse_float(8)?),
                max_luminance: parse_u32(9)?,
                min_luminance: parse_float(10)?,
            })
        } else {
            Err(Error::parse("Invalid master display format".to_string()))
        }
    }

    /// Generate x265 master-display parameter string
    pub fn format_master_display_for_x265(md: &MasteringDisplayColorVolume) -> String {
        format!(
            "G({:.4},{:.4})B({:.4},{:.4})R({:.4},{:.4})WP({:.4},{:.4})L({},{:.4})",
            md.green_primary.0, md.green_primary.1,
            md.blue_primary.0, md.blue_primary.1,
            md.red_primary.0, md.red_primary.1,
            md.white_point.0, md.white_point.1,
            md.max_luminance, md.min_luminance
        )
    }

    /// Parse content light level from raw string
    pub fn parse_content_light_level(raw: &str) -> Result<ContentLightLevelInfo> {
        // Expected format: "1000,400" (max_cll, max_fall)
        let parts: Vec<&str> = raw.split(',').collect();
        if parts.len() >= 2 {
            let max_cll = parts[0].trim().parse()
                .map_err(|e| Error::parse(format!("Failed to parse max_cll: {}", e)))?;
            let max_fall = parts[1].trim().parse()
                .map_err(|e| Error::parse(format!("Failed to parse max_fall: {}", e)))?;
            
            Ok(ContentLightLevelInfo {
                max_cll,
                max_fall,
            })
        } else if parts.len() == 1 {
            // Only max_cll provided
            let max_cll = parts[0].trim().parse()
                .map_err(|e| Error::parse(format!("Failed to parse max_cll: {}", e)))?;
            
            Ok(ContentLightLevelInfo {
                max_cll,
                max_fall: 400, // Default reasonable value
            })
        } else {
            Err(Error::parse("Invalid content light level format".to_string()))
        }
    }

    /// Format content light level for x265
    pub fn format_content_light_level_for_x265(cll: &ContentLightLevelInfo) -> String {
        format!("{},{}", cll.max_cll, cll.max_fall)
    }

    /// Validate HDR metadata for encoding
    pub fn validate_hdr_metadata(metadata: &HdrMetadata) -> Result<()> {
        match metadata.format {
            HdrFormat::None => {
                // SDR should not have HDR metadata
                if metadata.master_display.is_some() || metadata.content_light_level.is_some() {
                    return Err(Error::validation("SDR content should not have HDR metadata".to_string()));
                }
            },
            HdrFormat::HDR10 | HdrFormat::HDR10Plus => {
                // HDR10 should have proper transfer function
                if !matches!(metadata.transfer_function, TransferFunction::Smpte2084) {
                    return Err(Error::validation("HDR10 content should use SMPTE-2084 transfer function".to_string()));
                }

                // Validate color space
                if !matches!(metadata.color_space, ColorSpace::Bt2020) {
                    return Err(Error::validation("HDR10 content should use BT.2020 color space".to_string()));
                }

                // Validate mastering display metadata if present
                if let Some(ref md) = metadata.master_display {
                    Self::validate_mastering_display(md)?;
                }

                // Validate content light level if present
                if let Some(ref cll) = metadata.content_light_level {
                    Self::validate_content_light_level(cll)?;
                }
            },
            HdrFormat::HLG => {
                // HLG should have proper transfer function
                if !matches!(metadata.transfer_function, TransferFunction::AribStdB67) {
                    return Err(Error::validation("HLG content should use ARIB STD-B67 transfer function".to_string()));
                }

                // Validate color space
                if !matches!(metadata.color_space, ColorSpace::Bt2020) {
                    return Err(Error::validation("HLG content should use BT.2020 color space".to_string()));
                }
            },
        }

        Ok(())
    }

    fn validate_mastering_display(md: &MasteringDisplayColorVolume) -> Result<()> {
        // Validate chromaticity coordinates (should be between 0 and 1)
        let coords = [
            md.red_primary.0, md.red_primary.1,
            md.green_primary.0, md.green_primary.1,
            md.blue_primary.0, md.blue_primary.1,
            md.white_point.0, md.white_point.1,
        ];

        for (i, coord) in coords.iter().enumerate() {
            if *coord < 0.0 || *coord > 1.0 {
                return Err(Error::validation(format!(
                    "Chromaticity coordinate {} out of range [0.0, 1.0]: {}", i, coord
                )));
            }
        }

        // Validate luminance values
        if md.max_luminance == 0 {
            return Err(Error::validation("Max luminance must be greater than 0".to_string()));
        }

        if md.min_luminance < 0.0 {
            return Err(Error::validation("Min luminance must be non-negative".to_string()));
        }

        if md.min_luminance >= md.max_luminance as f32 {
            return Err(Error::validation("Min luminance must be less than max luminance".to_string()));
        }

        // Check for reasonable HDR range
        if md.max_luminance < 100 {
            return Err(Error::validation("Max luminance too low for HDR content".to_string()));
        }

        if md.max_luminance > 10000 {
            return Err(Error::validation("Max luminance unreasonably high".to_string()));
        }

        Ok(())
    }

    fn validate_content_light_level(cll: &ContentLightLevelInfo) -> Result<()> {
        if cll.max_cll == 0 {
            return Err(Error::validation("Max CLL must be greater than 0".to_string()));
        }

        if cll.max_fall == 0 {
            return Err(Error::validation("Max FALL must be greater than 0".to_string()));
        }

        if cll.max_fall > cll.max_cll {
            return Err(Error::validation("Max FALL cannot exceed Max CLL".to_string()));
        }

        // Check for reasonable HDR range
        if cll.max_cll < 100 {
            return Err(Error::validation("Max CLL too low for HDR content".to_string()));
        }

        if cll.max_cll > 10000 {
            return Err(Error::validation("Max CLL unreasonably high".to_string()));
        }

        Ok(())
    }

    /// Get appropriate default metadata for HDR format
    pub fn get_default_metadata_for_format(format: HdrFormat) -> HdrMetadata {
        match format {
            HdrFormat::None => HdrMetadata::sdr_default(),
            HdrFormat::HDR10 | HdrFormat::HDR10Plus => HdrMetadata::hdr10_default(),
            HdrFormat::HLG => HdrMetadata {
                format: HdrFormat::HLG,
                color_space: ColorSpace::Bt2020,
                transfer_function: TransferFunction::AribStdB67,
                color_primaries: ColorSpace::Bt2020,
                master_display: None, // HLG typically doesn't use mastering display
                content_light_level: None,
                raw_color_space: Some("bt2020nc".to_string()),
                raw_transfer: Some("arib-std-b67".to_string()),
                raw_primaries: Some("bt2020".to_string()),
            },
        }
    }

    /// Extract HDR metadata summary for logging
    pub fn get_metadata_summary(metadata: &HdrMetadata) -> String {
        let format_str = match metadata.format {
            HdrFormat::None => "SDR",
            HdrFormat::HDR10 => "HDR10",
            HdrFormat::HDR10Plus => "HDR10+",
            HdrFormat::HLG => "HLG",
        };

        let color_info = format!(
            "{:?}/{:?}", 
            metadata.color_space, 
            metadata.transfer_function
        );

        let metadata_info = match (&metadata.master_display, &metadata.content_light_level) {
            (Some(md), Some(cll)) => format!(
                " | MD: L({}-{:.2}) | CLL: {}/{}", 
                md.max_luminance, md.min_luminance,
                cll.max_cll, cll.max_fall
            ),
            (Some(md), None) => format!(
                " | MD: L({}-{:.2})", 
                md.max_luminance, md.min_luminance
            ),
            (None, Some(cll)) => format!(
                " | CLL: {}/{}", 
                cll.max_cll, cll.max_fall
            ),
            (None, None) => String::new(),
        };

        format!("{} [{}{}]", format_str, color_info, metadata_info)
    }
}