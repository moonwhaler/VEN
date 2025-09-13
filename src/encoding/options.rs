use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EncodingOptions {
    pub input_path: PathBuf,
    pub output_path: PathBuf,
    pub profile_name: String,
    pub title: Option<String>,
    pub mode: String,
    pub crop: Option<String>,
    pub denoise: bool,
    pub deinterlace: bool,
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
            denoise: false,
            deinterlace: false,
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

    pub fn with_denoise(mut self, enabled: bool) -> Self {
        self.denoise = enabled;
        self
    }

    pub fn with_deinterlace(mut self, enabled: bool) -> Self {
        self.deinterlace = enabled;
        self
    }

    pub fn is_auto_profile(&self) -> bool {
        self.profile_name == "auto"
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

        Ok(())
    }

    fn is_valid_crop_format(&self, crop: &str) -> bool {
        let parts: Vec<&str> = crop.split(':').collect();
        if parts.len() != 4 {
            return false;
        }

        parts.iter().all(|part| part.parse::<u32>().is_ok())
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
    use std::path::Path;
    use tempfile::NamedTempFile;

    #[test]
    fn test_encoding_options_builder() {
        let temp_input = NamedTempFile::new().unwrap();
        let options = EncodingOptions::new(temp_input.path(), Path::new("output.mkv"))
            .with_profile("anime".to_string())
            .with_mode("crf".to_string())
            .with_title("Test Movie".to_string())
            .with_crop(Some("1920:800:0:140".to_string()))
            .with_denoise(true);

        assert_eq!(options.profile_name, "anime");
        assert_eq!(options.mode, "crf");
        assert_eq!(options.title, Some("Test Movie".to_string()));
        assert_eq!(options.crop, Some("1920:800:0:140".to_string()));
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
}
