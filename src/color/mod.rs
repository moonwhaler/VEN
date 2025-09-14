pub mod spaces;
pub mod transfers;

pub use spaces::*;
pub use transfers::*;

use crate::hdr::types::{ColorSpace, TransferFunction};

/// Unified color management for video encoding
pub struct ColorManager;

impl ColorManager {
    /// Convert between color space representations
    pub fn normalize_color_space_name(raw_name: &str) -> ColorSpace {
        match raw_name.to_lowercase().as_str() {
            "bt709" | "rec709" | "bt.709" | "rec.709" => ColorSpace::Bt709,
            "bt2020" | "rec2020" | "bt.2020" | "rec.2020" | "bt2020nc" | "bt2020-ncl" => ColorSpace::Bt2020,
            "dci-p3" | "dcip3" | "p3-dci" => ColorSpace::DciP3,
            "display-p3" | "displayp3" | "p3-display" => ColorSpace::DisplayP3,
            _ => {
                tracing::warn!("Unknown color space '{}', defaulting to Bt709", raw_name);
                ColorSpace::Bt709
            }
        }
    }

    /// Convert between transfer function representations
    pub fn normalize_transfer_function_name(raw_name: &str) -> TransferFunction {
        match raw_name.to_lowercase().as_str() {
            "bt709" | "rec709" | "bt.709" | "rec.709" => TransferFunction::Bt709,
            "smpte2084" | "smpte-2084" | "st2084" | "st-2084" | "pq" => TransferFunction::Smpte2084,
            "arib-std-b67" | "arib_std_b67" | "aribstdb67" | "hlg" => TransferFunction::AribStdB67,
            "bt2020-10" | "bt.2020-10" | "rec2020-10" => TransferFunction::Bt2020_10,
            "bt2020-12" | "bt.2020-12" | "rec2020-12" => TransferFunction::Bt2020_12,
            _ => {
                tracing::warn!("Unknown transfer function '{}', defaulting to Bt709", raw_name);
                TransferFunction::Bt709
            }
        }
    }

    /// Check if color space and transfer function are compatible
    pub fn are_compatible(color_space: ColorSpace, transfer_function: TransferFunction) -> bool {
        match (color_space, transfer_function) {
            // SDR combinations
            (ColorSpace::Bt709, TransferFunction::Bt709) => true,
            
            // HDR10 combinations
            (ColorSpace::Bt2020, TransferFunction::Smpte2084) => true,
            (ColorSpace::Bt2020, TransferFunction::Bt2020_10) => true,
            (ColorSpace::Bt2020, TransferFunction::Bt2020_12) => true,
            
            // HLG combinations
            (ColorSpace::Bt2020, TransferFunction::AribStdB67) => true,
            
            // P3 combinations (typically SDR or tone-mapped HDR)
            (ColorSpace::DciP3, TransferFunction::Bt709) => true,
            (ColorSpace::DisplayP3, TransferFunction::Bt709) => true,
            
            // Cross-format combinations that might work but aren't standard
            _ => false,
        }
    }

    /// Get recommended color space for transfer function
    pub fn get_recommended_color_space(transfer_function: TransferFunction) -> ColorSpace {
        match transfer_function {
            TransferFunction::Bt709 => ColorSpace::Bt709,
            TransferFunction::Smpte2084 => ColorSpace::Bt2020,
            TransferFunction::AribStdB67 => ColorSpace::Bt2020,
            TransferFunction::Bt2020_10 => ColorSpace::Bt2020,
            TransferFunction::Bt2020_12 => ColorSpace::Bt2020,
        }
    }

    /// Get recommended transfer function for color space
    pub fn get_recommended_transfer_function(color_space: ColorSpace) -> TransferFunction {
        match color_space {
            ColorSpace::Bt709 => TransferFunction::Bt709,
            ColorSpace::Bt2020 => TransferFunction::Smpte2084, // Default to HDR10
            ColorSpace::DciP3 => TransferFunction::Bt709,
            ColorSpace::DisplayP3 => TransferFunction::Bt709,
        }
    }

    /// Get x265 parameter names for color space
    pub fn get_x265_color_space_params(color_space: ColorSpace) -> (&'static str, &'static str) {
        match color_space {
            ColorSpace::Bt709 => ("bt709", "bt709"),
            ColorSpace::Bt2020 => ("bt2020", "bt2020nc"),
            ColorSpace::DciP3 => ("bt709", "bt709"), // Approximate with bt709
            ColorSpace::DisplayP3 => ("bt709", "bt709"), // Approximate with bt709
        }
    }

    /// Get x265 parameter name for transfer function
    pub fn get_x265_transfer_function_param(transfer_function: TransferFunction) -> &'static str {
        match transfer_function {
            TransferFunction::Bt709 => "bt709",
            TransferFunction::Smpte2084 => "smpte2084",
            TransferFunction::AribStdB67 => "arib-std-b67",
            TransferFunction::Bt2020_10 => "bt2020-10",
            TransferFunction::Bt2020_12 => "bt2020-12",
        }
    }

    /// Validate color configuration for encoding
    pub fn validate_color_configuration(
        color_space: ColorSpace,
        transfer_function: TransferFunction,
        color_primaries: ColorSpace,
    ) -> Result<(), String> {
        // Check basic compatibility
        if !Self::are_compatible(color_space, transfer_function) {
            return Err(format!(
                "Color space {:?} is not compatible with transfer function {:?}",
                color_space, transfer_function
            ));
        }

        // Color primaries should match color space in most cases
        if color_space != color_primaries {
            tracing::warn!(
                "Color space {:?} differs from color primaries {:?}",
                color_space, color_primaries
            );
            // This is not necessarily an error, just unusual
        }

        // HDR-specific validation
        match transfer_function {
            TransferFunction::Smpte2084 | TransferFunction::AribStdB67 => {
                if !matches!(color_space, ColorSpace::Bt2020) {
                    return Err(format!(
                        "HDR transfer function {:?} should use Bt2020 color space",
                        transfer_function
                    ));
                }
            },
            _ => {}
        }

        Ok(())
    }
}