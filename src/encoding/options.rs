use std::path::PathBuf;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EncodingOptions {
    pub input_path: PathBuf,
    pub output_path: PathBuf,
    pub profile_name: String,
    pub title: Option<String>,
    pub mode: String,
    pub crop: Option<String>,
    pub scale: Option<String>,
    pub use_complexity_analysis: bool,
    pub denoise: bool,
    pub deinterlace: bool,
    pub web_search_enabled: bool,
    pub web_search_force: bool,
    pub no_web_search: bool,
}

impl EncodingOptions {
    pub fn new<P: Into<PathBuf>>(input_path: P, output_path: P) -> Self {
        Self {
            input_path: input_path.into(),
            output_path: output_path.into(),
            profile_name: "auto".to_string(),
            title: None,
            mode: "abr".to_string(),
            crop: None,
            scale: None,
            use_complexity_analysis: false,
            denoise: false,
            deinterlace: false,
            web_search_enabled: true,
            web_search_force: false,
            no_web_search: false,
        }
    }

    pub fn with_profile(mut self, profile: String) -> Self {
        self.profile_name = profile;
        self
    }

    pub fn with_mode(mut self, mode: String) -> Self {
        self.mode = mode;
        self
    }

    pub fn with_title(mut self, title: String) -> Self {
        self.title = Some(title);
        self
    }

    pub fn with_crop(mut self, crop: Option<String>) -> Self {
        self.crop = crop;
        self
    }

    pub fn with_scale(mut self, scale: Option<String>) -> Self {
        self.scale = scale;
        self
    }

    pub fn with_complexity_analysis(mut self, enabled: bool) -> Self {
        self.use_complexity_analysis = enabled;
        self
    }

    pub fn with_denoise(mut self, enabled: bool) -> Self {
        self.denoise = enabled;
        self
    }

    pub fn with_deinterlace(mut self, enabled: bool) -> Self {
        self.deinterlace = enabled;
        self
    }


    pub fn with_web_search(mut self, enabled: bool) -> Self {
        self.web_search_enabled = enabled;
        self
    }

    pub fn with_web_search_force(mut self, force: bool) -> Self {
        self.web_search_force = force;
        self
    }

    pub fn with_no_web_search(mut self, disabled: bool) -> Self {
        self.no_web_search = disabled;
        self
    }

    pub fn is_auto_profile(&self) -> bool {
        self.profile_name == "auto"
    }

    pub fn should_use_web_search(&self) -> bool {
        if self.no_web_search {
            false
        } else if self.web_search_force {
            true
        } else {
            self.web_search_enabled
        }
    }

    pub fn validate(&self) -> crate::utils::Result<()> {
        if !self.input_path.exists() {
            return Err(crate::utils::Error::validation(format!(
                "Input file does not exist: {}",
                self.input_path.display()
            )));
        }

        if !["crf", "abr", "cbr"].contains(&self.mode.as_str()) {
            return Err(crate::utils::Error::validation(format!(
                "Invalid encoding mode: {} (must be crf, abr, or cbr)",
                self.mode
            )));
        }

        if let Some(crop) = &self.crop {
            if !self.is_valid_crop_format(crop) {
                return Err(crate::utils::Error::validation(format!(
                    "Invalid crop format: {} (expected format: width:height:x:y)",
                    crop
                )));
            }
        }

        if let Some(scale) = &self.scale {
            if !self.is_valid_scale_format(scale) {
                return Err(crate::utils::Error::validation(format!(
                    "Invalid scale format: {} (expected format: widthxheight)",
                    scale
                )));
            }
        }

        Ok(())
    }

    fn is_valid_crop_format(&self, crop: &str) -> bool {
        let parts: Vec<&str> = crop.split(':').collect();
        if parts.len() != 4 {
            return false;
        }

        parts.iter().all(|part| part.parse::<u32>().is_ok())
    }

    fn is_valid_scale_format(&self, scale: &str) -> bool {
        let parts: Vec<&str> = scale.split('x').collect();
        if parts.len() != 2 {
            return false;
        }

        parts.iter().all(|part| {
            *part == "-1" || part.parse::<u32>().is_ok()
        })
    }
}

impl Default for EncodingOptions {
    fn default() -> Self {
        Self::new("input.mkv", "output.mkv")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    #[test]
    fn test_encoding_options_builder() {
        let temp_input = NamedTempFile::new().unwrap();
        let options = EncodingOptions::new(temp_input.path(), "output.mkv")
            .with_profile("anime".to_string())
            .with_mode("crf".to_string())
            .with_title("Test Movie".to_string())
            .with_crop(Some("1920:800:0:140".to_string()))
            .with_complexity_analysis(true)
            .with_denoise(true);

        assert_eq!(options.profile_name, "anime");
        assert_eq!(options.mode, "crf");
        assert_eq!(options.title, Some("Test Movie".to_string()));
        assert_eq!(options.crop, Some("1920:800:0:140".to_string()));
        assert!(options.use_complexity_analysis);
        assert!(options.denoise);
    }

    #[test]
    fn test_validate_crop_format() {
        let options = EncodingOptions::default();
        
        assert!(options.is_valid_crop_format("1920:800:0:140"));
        assert!(options.is_valid_crop_format("1280:720:0:0"));
        assert!(!options.is_valid_crop_format("1920:800:0"));
        assert!(!options.is_valid_crop_format("1920:800:0:abc"));
        assert!(!options.is_valid_crop_format("invalid"));
    }

    #[test]
    fn test_validate_scale_format() {
        let options = EncodingOptions::default();
        
        assert!(options.is_valid_scale_format("1920x1080"));
        assert!(options.is_valid_scale_format("1280x-1"));
        assert!(options.is_valid_scale_format("-1x720"));
        assert!(!options.is_valid_scale_format("1920"));
        assert!(!options.is_valid_scale_format("1920xabc"));
        assert!(!options.is_valid_scale_format("invalid"));
    }

    #[test]
    fn test_web_search_logic() {
        let mut options = EncodingOptions::default();
        
        // Default behavior
        assert!(options.should_use_web_search());
        
        // Force enabled
        options = options.with_web_search_force(true);
        assert!(options.should_use_web_search());
        
        // Disabled
        options = options.with_no_web_search(true);
        assert!(!options.should_use_web_search());
        
        // Disabled overrides force
        options = options.with_web_search_force(true).with_no_web_search(true);
        assert!(!options.should_use_web_search());
    }
}