use crate::hdr::types::ColorSpace;

/// Color space definitions and utilities
pub struct ColorSpaceInfo {
    pub name: &'static str,
    pub display_name: &'static str,
    pub description: &'static str,
    pub gamut_coverage: f32, // Approximate coverage of visible spectrum (0.0-1.0)
    pub is_hdr_compatible: bool,
}

impl ColorSpace {
    /// Get detailed information about a color space
    pub fn info(&self) -> ColorSpaceInfo {
        match self {
            ColorSpace::Bt709 => ColorSpaceInfo {
                name: "bt709",
                display_name: "BT.709 (Rec.709)",
                description: "Standard definition and HD television color space",
                gamut_coverage: 0.35,
                is_hdr_compatible: false,
            },
            ColorSpace::Bt2020 => ColorSpaceInfo {
                name: "bt2020",
                display_name: "BT.2020 (Rec.2020)",
                description: "Ultra HD and HDR television color space with wide gamut",
                gamut_coverage: 0.76,
                is_hdr_compatible: true,
            },
            ColorSpace::DciP3 => ColorSpaceInfo {
                name: "dci-p3",
                display_name: "DCI-P3",
                description: "Digital cinema color space",
                gamut_coverage: 0.53,
                is_hdr_compatible: false,
            },
            ColorSpace::DisplayP3 => ColorSpaceInfo {
                name: "display-p3",
                display_name: "Display P3",
                description: "Apple display color space, similar to DCI-P3",
                gamut_coverage: 0.53,
                is_hdr_compatible: false,
            },
        }
    }

    /// Get the primary chromaticity coordinates
    pub fn get_primaries(&self) -> ColorPrimaries {
        match self {
            ColorSpace::Bt709 => ColorPrimaries {
                red: (0.64, 0.33),
                green: (0.30, 0.60),
                blue: (0.15, 0.06),
                white_point: (0.3127, 0.3290), // D65
            },
            ColorSpace::Bt2020 => ColorPrimaries {
                red: (0.708, 0.292),
                green: (0.170, 0.797),
                blue: (0.131, 0.046),
                white_point: (0.3127, 0.3290), // D65
            },
            ColorSpace::DciP3 => ColorPrimaries {
                red: (0.680, 0.320),
                green: (0.265, 0.690),
                blue: (0.150, 0.060),
                white_point: (0.314, 0.351), // DCI white point
            },
            ColorSpace::DisplayP3 => ColorPrimaries {
                red: (0.680, 0.320),
                green: (0.265, 0.690),
                blue: (0.150, 0.060),
                white_point: (0.3127, 0.3290), // D65 (different from DCI-P3)
            },
        }
    }

    /// Get all supported color spaces
    pub fn all() -> Vec<ColorSpace> {
        vec![
            ColorSpace::Bt709,
            ColorSpace::Bt2020,
            ColorSpace::DciP3,
            ColorSpace::DisplayP3,
        ]
    }

    /// Get HDR-compatible color spaces
    pub fn hdr_compatible() -> Vec<ColorSpace> {
        Self::all()
            .into_iter()
            .filter(|cs| cs.info().is_hdr_compatible)
            .collect()
    }

    /// Check if color space is wide gamut
    pub fn is_wide_gamut(&self) -> bool {
        self.info().gamut_coverage > 0.45
    }

    /// Get the closest compatible color space for a given gamut coverage
    pub fn find_best_for_gamut(target_coverage: f32) -> ColorSpace {
        let mut best_space = ColorSpace::Bt709;
        let mut best_diff = f32::MAX;

        for space in Self::all() {
            let coverage = space.info().gamut_coverage;
            let diff = (coverage - target_coverage).abs();
            if diff < best_diff {
                best_diff = diff;
                best_space = space;
            }
        }

        best_space
    }
}

/// Color primaries with chromaticity coordinates
#[derive(Debug, Clone, PartialEq)]
pub struct ColorPrimaries {
    pub red: (f32, f32),
    pub green: (f32, f32),
    pub blue: (f32, f32),
    pub white_point: (f32, f32),
}

impl ColorPrimaries {
    /// Calculate the color gamut area (approximate)
    pub fn gamut_area(&self) -> f32 {
        // Use the triangle area formula for the RGB triangle
        let r = self.red;
        let g = self.green;
        let b = self.blue;

        0.5 * ((r.0 * (g.1 - b.1) + g.0 * (b.1 - r.1) + b.0 * (r.1 - g.1)).abs())
    }

    /// Check if primaries are valid (within [0,1] range)
    pub fn validate(&self) -> Result<(), String> {
        let coords = [
            ("red.x", self.red.0),
            ("red.y", self.red.1),
            ("green.x", self.green.0),
            ("green.y", self.green.1),
            ("blue.x", self.blue.0),
            ("blue.y", self.blue.1),
            ("white_point.x", self.white_point.0),
            ("white_point.y", self.white_point.1),
        ];

        for (name, value) in coords {
            if !(0.0..=1.0).contains(&value) {
                return Err(format!(
                    "Chromaticity coordinate {} out of range [0,1]: {}",
                    name, value
                ));
            }
        }

        // Check if the RGB triangle is valid (positive area)
        if self.gamut_area() <= 0.0 {
            return Err(
                "Invalid color primaries: RGB triangle has zero or negative area".to_string(),
            );
        }

        Ok(())
    }
}
