use crate::hdr::types::TransferFunction;

/// Transfer function definitions and utilities
pub struct TransferFunctionInfo {
    pub name: &'static str,
    pub display_name: &'static str,
    pub description: &'static str,
    pub gamma_approximation: f32, // Approximate gamma value for display
    pub peak_luminance: f32,      // Peak luminance in nits
    pub is_hdr: bool,
}

impl TransferFunction {
    /// Get detailed information about a transfer function
    pub fn info(&self) -> TransferFunctionInfo {
        match self {
            TransferFunction::Bt709 => TransferFunctionInfo {
                name: "bt709",
                display_name: "BT.709 (Rec.709)",
                description: "Standard gamma transfer function for SDR content",
                gamma_approximation: 2.4,
                peak_luminance: 100.0,
                is_hdr: false,
            },
            TransferFunction::Smpte2084 => TransferFunctionInfo {
                name: "smpte2084",
                display_name: "SMPTE-2084 (PQ)",
                description: "Perceptual Quantizer for HDR10 content",
                gamma_approximation: 0.0, // Non-gamma curve
                peak_luminance: 10000.0,
                is_hdr: true,
            },
            TransferFunction::AribStdB67 => TransferFunctionInfo {
                name: "arib-std-b67",
                display_name: "ARIB STD-B67 (HLG)",
                description: "Hybrid Log-Gamma for HDR broadcast content",
                gamma_approximation: 1.2, // Variable gamma
                peak_luminance: 1000.0,
                is_hdr: true,
            },
            TransferFunction::Bt2020_10 => TransferFunctionInfo {
                name: "bt2020-10",
                display_name: "BT.2020 (10-bit)",
                description: "BT.2020 transfer function with 10-bit quantization",
                gamma_approximation: 2.4,
                peak_luminance: 100.0,
                is_hdr: false,
            },
            TransferFunction::Bt2020_12 => TransferFunctionInfo {
                name: "bt2020-12",
                display_name: "BT.2020 (12-bit)",
                description: "BT.2020 transfer function with 12-bit quantization",
                gamma_approximation: 2.4,
                peak_luminance: 100.0,
                is_hdr: false,
            },
        }
    }

    /// Get all supported transfer functions
    pub fn all() -> Vec<TransferFunction> {
        vec![
            TransferFunction::Bt709,
            TransferFunction::Smpte2084,
            TransferFunction::AribStdB67,
            TransferFunction::Bt2020_10,
            TransferFunction::Bt2020_12,
        ]
    }

    /// Get HDR transfer functions
    pub fn hdr_functions() -> Vec<TransferFunction> {
        Self::all()
            .into_iter()
            .filter(|tf| tf.info().is_hdr)
            .collect()
    }

    /// Check if transfer function supports HDR
    pub fn is_hdr(&self) -> bool {
        self.info().is_hdr
    }

    /// Get recommended bit depth for transfer function
    pub fn recommended_bit_depth(&self) -> u8 {
        match self {
            TransferFunction::Bt709 => 8,
            TransferFunction::Smpte2084 => 10,
            TransferFunction::AribStdB67 => 10,
            TransferFunction::Bt2020_10 => 10,
            TransferFunction::Bt2020_12 => 12,
        }
    }

    /// Get minimum required bit depth for transfer function
    pub fn minimum_bit_depth(&self) -> u8 {
        match self {
            TransferFunction::Bt709 => 8,
            TransferFunction::Smpte2084 => 10, // PQ requires at least 10-bit
            TransferFunction::AribStdB67 => 10, // HLG requires at least 10-bit
            TransferFunction::Bt2020_10 => 10,
            TransferFunction::Bt2020_12 => 12,
        }
    }

    /// Get the luminance range for the transfer function
    pub fn luminance_range(&self) -> (f32, f32) {
        match self {
            TransferFunction::Bt709 => (0.0, 100.0),
            TransferFunction::Smpte2084 => (0.0, 10000.0),
            TransferFunction::AribStdB67 => (0.0, 1000.0),
            TransferFunction::Bt2020_10 => (0.0, 100.0),
            TransferFunction::Bt2020_12 => (0.0, 100.0),
        }
    }

    /// Check if bit depth is sufficient for transfer function
    pub fn is_bit_depth_sufficient(&self, bit_depth: u8) -> bool {
        bit_depth >= self.minimum_bit_depth()
    }

    /// Get encoding complexity factor
    pub fn encoding_complexity_factor(&self) -> f32 {
        match self {
            TransferFunction::Bt709 => 1.0,
            TransferFunction::Smpte2084 => 1.3, // PQ encoding is more complex
            TransferFunction::AribStdB67 => 1.2, // HLG encoding is moderately complex
            TransferFunction::Bt2020_10 => 1.1,
            TransferFunction::Bt2020_12 => 1.15,
        }
    }

    /// Find best transfer function for target peak luminance
    pub fn find_best_for_luminance(peak_luminance: f32) -> TransferFunction {
        if peak_luminance <= 100.0 {
            TransferFunction::Bt709
        } else if peak_luminance <= 1000.0 {
            TransferFunction::AribStdB67 // HLG is good for moderate HDR
        } else {
            TransferFunction::Smpte2084 // PQ for high luminance HDR
        }
    }

    /// Get transfer function characteristics for encoding optimization
    pub fn get_encoding_characteristics(&self) -> TransferCharacteristics {
        match self {
            TransferFunction::Bt709 => TransferCharacteristics {
                requires_tone_mapping: false,
                supports_dynamic_metadata: false,
                optimal_quantization: QuantizationStrategy::Uniform,
                perceptual_weighting: 1.0,
            },
            TransferFunction::Smpte2084 => TransferCharacteristics {
                requires_tone_mapping: true,
                supports_dynamic_metadata: true,
                optimal_quantization: QuantizationStrategy::Perceptual,
                perceptual_weighting: 1.3,
            },
            TransferFunction::AribStdB67 => TransferCharacteristics {
                requires_tone_mapping: false, // HLG is backward compatible
                supports_dynamic_metadata: false,
                optimal_quantization: QuantizationStrategy::Hybrid,
                perceptual_weighting: 1.1,
            },
            TransferFunction::Bt2020_10 | TransferFunction::Bt2020_12 => TransferCharacteristics {
                requires_tone_mapping: false,
                supports_dynamic_metadata: false,
                optimal_quantization: QuantizationStrategy::Uniform,
                perceptual_weighting: 1.05,
            },
        }
    }
}

/// Transfer function encoding characteristics
#[derive(Debug, Clone)]
pub struct TransferCharacteristics {
    pub requires_tone_mapping: bool,
    pub supports_dynamic_metadata: bool,
    pub optimal_quantization: QuantizationStrategy,
    pub perceptual_weighting: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub enum QuantizationStrategy {
    Uniform,    // Even distribution of quantization levels
    Perceptual, // More levels in perceptually important ranges
    Hybrid,     // Combination approach
}

/// Utilities for transfer function conversion and validation
pub struct TransferFunctionUtils;

impl TransferFunctionUtils {
    /// Convert from EOTF (Electro-Optical Transfer Function) name
    pub fn from_eotf_name(eotf_name: &str) -> Option<TransferFunction> {
        match eotf_name.to_lowercase().as_str() {
            "bt1886" | "rec1886" => Some(TransferFunction::Bt709),
            "pq" | "smpte2084" | "st2084" => Some(TransferFunction::Smpte2084),
            "hlg" | "arib-std-b67" => Some(TransferFunction::AribStdB67),
            "bt2020" => Some(TransferFunction::Bt2020_10),
            _ => None,
        }
    }

    /// Get EOTF name for transfer function
    pub fn to_eotf_name(tf: TransferFunction) -> &'static str {
        match tf {
            TransferFunction::Bt709 => "bt1886",
            TransferFunction::Smpte2084 => "pq",
            TransferFunction::AribStdB67 => "hlg",
            TransferFunction::Bt2020_10 => "bt2020-10",
            TransferFunction::Bt2020_12 => "bt2020-12",
        }
    }

    /// Validate transfer function parameters for encoding
    pub fn validate_for_encoding(
        tf: TransferFunction,
        bit_depth: u8,
        target_luminance: Option<f32>,
    ) -> Result<(), String> {
        // Check bit depth requirements
        if !tf.is_bit_depth_sufficient(bit_depth) {
            return Err(format!(
                "Bit depth {} insufficient for {:?}, minimum required: {}",
                bit_depth,
                tf,
                tf.minimum_bit_depth()
            ));
        }

        // Check luminance range compatibility
        if let Some(luminance) = target_luminance {
            let (_min_lum, max_lum) = tf.luminance_range();
            if luminance > max_lum {
                return Err(format!(
                    "Target luminance {} exceeds maximum for {:?}: {}",
                    luminance, tf, max_lum
                ));
            }
        }

        Ok(())
    }
}
