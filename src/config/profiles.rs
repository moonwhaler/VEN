use super::types::{ContentType, RawProfile};
use crate::analysis::dolby_vision::{DolbyVisionInfo, DolbyVisionProfile};
use crate::dolby_vision::RpuMetadata;
use crate::utils::{Error, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EncodingProfile {
    pub name: String,
    pub title: String,
    pub base_crf: f32,
    pub base_bitrate: u32,
    pub hdr_bitrate: u32,
    pub content_type: ContentType,
    pub x265_params: HashMap<String, String>,
}

impl EncodingProfile {
    pub fn from_raw(name: String, raw: RawProfile) -> Result<Self> {
        let content_type = ContentType::from_string(&raw.content_type)
            .ok_or_else(|| Error::profile(format!("Invalid content type: {}", raw.content_type)))?;

        let x265_params = raw
            .x265_params
            .into_iter()
            .map(|(k, v)| {
                let value_str = match v {
                    serde_yaml::Value::String(s) => s,
                    serde_yaml::Value::Number(n) => n.to_string(),
                    serde_yaml::Value::Bool(b) => {
                        if b {
                            "1".to_string()
                        } else {
                            "0".to_string()
                        }
                    }
                    _ => {
                        return Err(Error::profile(format!(
                            "Unsupported parameter value type for {}: {:?}",
                            k, v
                        )));
                    }
                };
                Ok((k, value_str))
            })
            .collect::<Result<HashMap<String, String>>>()?;

        Ok(EncodingProfile {
            name,
            title: raw.title,
            base_crf: raw.base_crf,
            base_bitrate: raw.base_bitrate,
            hdr_bitrate: raw.hdr_bitrate,
            content_type,
            x265_params,
        })
    }

    pub fn calculate_adaptive_crf(
        &self,
        crf_modifier: f32,
        is_hdr: bool,
        hdr_crf_adjustment: f32,
    ) -> f32 {
        let mut crf = self.base_crf + crf_modifier;
        if is_hdr {
            crf += hdr_crf_adjustment;
        }
        crf.clamp(1.0, 51.0)
    }

    pub fn calculate_adaptive_bitrate(&self, bitrate_multiplier: f32, is_hdr: bool) -> u32 {
        let base = if is_hdr {
            self.hdr_bitrate
        } else {
            self.base_bitrate
        };
        (base as f32 * bitrate_multiplier) as u32
    }

    pub fn build_x265_params_string(
        &self,
        mode_specific_params: Option<&HashMap<String, String>>,
    ) -> String {
        self.build_x265_params_string_with_hdr(
            mode_specific_params,
            None,
            None,
            None,
            None,
            None,
            None,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn build_x265_params_string_with_hdr(
        &self,
        mode_specific_params: Option<&HashMap<String, String>>,
        is_hdr: Option<bool>,
        color_space: Option<&String>,
        transfer_function: Option<&String>,
        color_primaries: Option<&String>,
        master_display: Option<&String>,
        max_cll: Option<&String>,
    ) -> String {
        let mut params = self.x265_params.clone();

        if let Some(mode_params) = mode_specific_params {
            for (key, value) in mode_params {
                params.insert(key.clone(), value.clone());
            }
        }

        // Inject HDR-specific parameters if HDR content is detected
        if is_hdr.unwrap_or(false) {
            // Map color_space to colormatrix parameter
            if let Some(cs) = color_space {
                if cs.contains("bt2020") || cs.contains("rec2020") {
                    params.insert("colormatrix".to_string(), "bt2020nc".to_string());
                }
            }

            // Map transfer_function to transfer parameter
            if let Some(tf) = transfer_function {
                if tf.contains("smpte2084") {
                    params.insert("transfer".to_string(), "smpte2084".to_string());
                } else if tf.contains("arib-std-b67") {
                    params.insert("transfer".to_string(), "arib-std-b67".to_string());
                }
            }

            // Map color_primaries to colorprim parameter
            if let Some(cp) = color_primaries {
                if cp.contains("bt2020") || cp.contains("rec2020") {
                    params.insert("colorprim".to_string(), "bt2020".to_string());
                }
            }

            // Add master-display metadata if available
            if let Some(md) = master_display {
                params.insert("master-display".to_string(), md.clone());
            }

            // Add max-cll metadata if available
            if let Some(cll) = max_cll {
                params.insert("max-cll".to_string(), format!("{},400", cll));
            }
        }

        let param_strs: Vec<String> = params
            .into_iter()
            .map(|(key, value)| {
                if value.is_empty() || value == "true" || value == "1" {
                    key
                } else {
                    format!("{}={}", key, value)
                }
            })
            .collect();

        param_strs.join(":")
    }

    /// Build x265 parameters with Dolby Vision support
    #[allow(clippy::too_many_arguments)]
    pub fn build_x265_params_string_with_dolby_vision(
        &self,
        mode_specific_params: Option<&HashMap<String, String>>,
        is_hdr: Option<bool>,
        color_space: Option<&String>,
        transfer_function: Option<&String>,
        color_primaries: Option<&String>,
        master_display: Option<&String>,
        max_cll: Option<&String>,
        dv_info: Option<&DolbyVisionInfo>,
        rpu_metadata: Option<&RpuMetadata>,
    ) -> String {
        let mut params = self.x265_params.clone();

        // Add mode-specific parameters first
        if let Some(mode_params) = mode_specific_params {
            for (key, value) in mode_params {
                params.insert(key.clone(), value.clone());
            }
        }

        // Add HDR parameters if HDR content is detected
        if is_hdr.unwrap_or(false) {
            // Map color_space to colormatrix parameter
            if let Some(cs) = color_space {
                if cs.contains("bt2020") || cs.contains("rec2020") {
                    params.insert("colormatrix".to_string(), "bt2020nc".to_string());
                }
            }

            // Map transfer_function to transfer parameter
            if let Some(tf) = transfer_function {
                if tf.contains("smpte2084") {
                    params.insert("transfer".to_string(), "smpte2084".to_string());
                } else if tf.contains("arib-std-b67") {
                    params.insert("transfer".to_string(), "arib-std-b67".to_string());
                }
            }

            // Map color_primaries to colorprim parameter
            if let Some(cp) = color_primaries {
                if cp.contains("bt2020") || cp.contains("rec2020") {
                    params.insert("colorprim".to_string(), "bt2020".to_string());
                }
            }

            // Add master-display metadata if available
            if let Some(md) = master_display {
                params.insert("master-display".to_string(), md.clone());
            }

            // Add max-cll metadata if available
            if let Some(cll) = max_cll {
                params.insert("max-cll".to_string(), format!("{},400", cll));
            }
        }

        // Add Dolby Vision specific parameters if DV content is detected
        if let (Some(dv_info), Some(rpu_meta)) = (dv_info, rpu_metadata) {
            if dv_info.is_dolby_vision() && rpu_meta.extracted_successfully {
                // Add RPU file path
                params.insert(
                    "dolby-vision-rpu".to_string(),
                    rpu_meta.temp_file.to_string_lossy().to_string(),
                );

                // Add Dolby Vision profile
                match rpu_meta.profile {
                    DolbyVisionProfile::Profile5 => {
                        params.insert("dolby-vision-profile".to_string(), "5".to_string());
                    },
                    DolbyVisionProfile::Profile81 => {
                        params.insert("dolby-vision-profile".to_string(), "8.1".to_string());
                    },
                    DolbyVisionProfile::Profile82 => {
                        params.insert("dolby-vision-profile".to_string(), "8.2".to_string());
                    },
                    DolbyVisionProfile::Profile84 => {
                        params.insert("dolby-vision-profile".to_string(), "8.4".to_string());
                    },
                    _ => {} // Skip profile 7 and others not directly supported by x265
                }

                // Ensure appropriate VBV settings for Dolby Vision
                if !params.contains_key("vbv-bufsize") {
                    params.insert("vbv-bufsize".to_string(), "20000".to_string());
                }
                if !params.contains_key("vbv-maxrate") {
                    params.insert("vbv-maxrate".to_string(), "20000".to_string());
                }

                // Force 10-bit output for Dolby Vision
                params.insert("output-depth".to_string(), "10".to_string());
                
                // Ensure proper color parameters for Dolby Vision
                params.insert("colorprim".to_string(), "bt2020".to_string());
                params.insert("transfer".to_string(), "smpte2084".to_string());
                params.insert("colormatrix".to_string(), "bt2020nc".to_string());
            }
        }

        // Build parameter string
        let param_strs: Vec<String> = params
            .into_iter()
            .map(|(key, value)| {
                if value.is_empty() || value == "true" || value == "1" {
                    key
                } else {
                    format!("{}={}", key, value)
                }
            })
            .collect();

        param_strs.join(":")
    }

    /// Check if this profile is compatible with Dolby Vision encoding
    pub fn is_dolby_vision_compatible(&self) -> bool {
        // Check if the profile has 10-bit output and appropriate color settings
        let has_10bit = self.x265_params.get("output-depth")
            .map(|d| d == "10")
            .unwrap_or(false) || 
            self.x265_params.get("pix_fmt")
                .map(|pf| pf.contains("10le"))
                .unwrap_or(false);

        let has_main10_profile = self.x265_params.get("profile")
            .map(|p| p == "main10")
            .unwrap_or(false);

        has_10bit || has_main10_profile
    }
}

pub struct ProfileManager {
    profiles: HashMap<String, EncodingProfile>,
}

impl ProfileManager {
    pub fn new() -> Self {
        Self {
            profiles: HashMap::new(),
        }
    }

    pub fn load_profiles(&mut self, raw_profiles: HashMap<String, RawProfile>) -> Result<()> {
        self.profiles.clear();

        for (name, raw_profile) in raw_profiles {
            let profile = EncodingProfile::from_raw(name.clone(), raw_profile)?;
            self.profiles.insert(name, profile);
        }

        Ok(())
    }

    pub fn get_profile(&self, name: &str) -> Option<&EncodingProfile> {
        self.profiles.get(name)
    }

    pub fn list_profiles(&self) -> Vec<&String> {
        self.profiles.keys().collect()
    }

    pub fn get_profiles_by_content_type(&self, content_type: ContentType) -> Vec<&EncodingProfile> {
        self.profiles
            .values()
            .filter(|p| p.content_type == content_type)
            .collect()
    }

    pub fn recommend_profile_for_resolution(
        &self,
        width: u32,
        height: u32,
        content_type: ContentType,
    ) -> Option<&EncodingProfile> {
        let profiles = self.get_profiles_by_content_type(content_type);

        if profiles.is_empty() {
            return None;
        }

        let is_4k = width >= 3840 || height >= 2160;

        match content_type {
            ContentType::Anime => self.get_profile("anime"),
            ContentType::ClassicAnime => self.get_profile("classic_anime"),
            ContentType::Animation3D => {
                if is_4k {
                    self.get_profile("3d_complex")
                        .or_else(|| self.get_profile("3d_cgi"))
                } else {
                    self.get_profile("3d_cgi")
                }
            }
            ContentType::Film => {
                if is_4k {
                    self.get_profile("movie")
                } else {
                    self.get_profile("movie_size_focused")
                        .or_else(|| self.get_profile("movie"))
                }
            }
            ContentType::HeavyGrain => self.get_profile("heavy_grain"),
            ContentType::LightGrain => self
                .get_profile("movie_mid_grain")
                .or_else(|| self.get_profile("movie")),
            ContentType::Action | ContentType::CleanDigital => {
                if is_4k {
                    self.get_profile("movie")
                } else {
                    self.get_profile("movie_size_focused")
                        .or_else(|| self.get_profile("movie"))
                }
            }
            ContentType::Mixed => {
                if is_4k {
                    self.get_profile("4k").or_else(|| self.get_profile("movie"))
                } else {
                    self.get_profile("movie")
                }
            }
        }
    }
}

impl Default for ProfileManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_yaml::Value;

    fn create_test_raw_profile() -> RawProfile {
        let mut x265_params = HashMap::new();
        x265_params.insert("preset".to_string(), Value::String("slow".to_string()));
        x265_params.insert(
            "crf".to_string(),
            Value::Number(serde_yaml::Number::from(22)),
        );
        x265_params.insert("weightb".to_string(), Value::Bool(true));
        x265_params.insert("no-sao".to_string(), Value::Bool(false));

        RawProfile {
            title: "Test Profile".to_string(),
            base_crf: 22.0,
            base_bitrate: 10000,
            hdr_bitrate: 13000,
            content_type: "film".to_string(),
            x265_params,
        }
    }

    #[test]
    fn test_encoding_profile_from_raw() {
        let raw = create_test_raw_profile();
        let profile = EncodingProfile::from_raw("test".to_string(), raw).unwrap();

        assert_eq!(profile.name, "test");
        assert_eq!(profile.title, "Test Profile");
        assert_eq!(profile.base_crf, 22.0);
        assert_eq!(profile.content_type, ContentType::Film);
        assert_eq!(profile.x265_params.get("preset"), Some(&"slow".to_string()));
        assert_eq!(profile.x265_params.get("crf"), Some(&"22".to_string()));
        assert_eq!(profile.x265_params.get("weightb"), Some(&"1".to_string()));
        assert_eq!(profile.x265_params.get("no-sao"), Some(&"0".to_string()));
    }

    #[test]
    fn test_calculate_adaptive_crf() {
        let raw = create_test_raw_profile();
        let profile = EncodingProfile::from_raw("test".to_string(), raw).unwrap();

        assert_eq!(profile.calculate_adaptive_crf(0.0, false, 2.0), 22.0);
        assert_eq!(profile.calculate_adaptive_crf(0.5, false, 2.0), 22.5);
        assert_eq!(profile.calculate_adaptive_crf(0.0, true, 2.0), 24.0);
        assert_eq!(profile.calculate_adaptive_crf(0.5, true, 2.0), 24.5);
    }

    #[test]
    fn test_calculate_adaptive_bitrate() {
        let raw = create_test_raw_profile();
        let profile = EncodingProfile::from_raw("test".to_string(), raw).unwrap();

        assert_eq!(profile.calculate_adaptive_bitrate(1.0, false), 10000);
        assert_eq!(profile.calculate_adaptive_bitrate(1.5, false), 15000);
        assert_eq!(profile.calculate_adaptive_bitrate(1.0, true), 13000);
        assert_eq!(profile.calculate_adaptive_bitrate(1.5, true), 19500);
    }

    #[test]
    fn test_build_x265_params_string() {
        let raw = create_test_raw_profile();
        let profile = EncodingProfile::from_raw("test".to_string(), raw).unwrap();

        let params_str = profile.build_x265_params_string(None);
        assert!(params_str.contains("preset=slow"));
        assert!(params_str.contains("crf=22"));
        assert!(params_str.contains("weightb"));
        assert!(params_str.contains("no-sao=0"));
    }

    #[test]
    fn test_hdr_parameter_injection() {
        let raw = create_test_raw_profile();
        let profile = EncodingProfile::from_raw("test".to_string(), raw).unwrap();

        let color_space = Some("bt2020nc".to_string());
        let transfer_function = Some("smpte2084".to_string());
        let color_primaries = Some("bt2020".to_string());
        let master_display = Some(
            "G(0.17,0.797)B(0.131,0.046)R(0.708,0.292)WP(0.3127,0.329)L(1000,0.01)".to_string(),
        );
        let max_cll = Some("1000".to_string());

        let params_str = profile.build_x265_params_string_with_hdr(
            None,
            Some(true), // is_hdr = true
            color_space.as_ref(),
            transfer_function.as_ref(),
            color_primaries.as_ref(),
            master_display.as_ref(),
            max_cll.as_ref(),
        );

        // Verify HDR-specific parameters are injected
        assert!(params_str.contains("colormatrix=bt2020nc"));
        assert!(params_str.contains("transfer=smpte2084"));
        assert!(params_str.contains("colorprim=bt2020"));
        assert!(params_str.contains(
            "master-display=G(0.17,0.797)B(0.131,0.046)R(0.708,0.292)WP(0.3127,0.329)L(1000,0.01)"
        ));
        assert!(params_str.contains("max-cll=1000,400"));
    }

    #[test]
    fn test_no_hdr_parameter_injection_when_sdr() {
        let raw = create_test_raw_profile();
        let profile = EncodingProfile::from_raw("test".to_string(), raw).unwrap();

        let params_str = profile.build_x265_params_string_with_hdr(
            None,
            Some(false), // is_hdr = false
            Some(&"bt709".to_string()),
            Some(&"bt709".to_string()),
            Some(&"bt709".to_string()),
            None,
            None,
        );

        // Verify HDR parameters are NOT injected for SDR content
        assert!(!params_str.contains("colormatrix=bt2020nc"));
        assert!(!params_str.contains("transfer=smpte2084"));
        assert!(!params_str.contains("colorprim=bt2020"));
        assert!(!params_str.contains("master-display"));
        assert!(!params_str.contains("max-cll"));
    }

    #[test]
    fn test_profile_manager() {
        let mut manager = ProfileManager::new();
        let mut profiles = HashMap::new();
        profiles.insert("test".to_string(), create_test_raw_profile());

        manager.load_profiles(profiles).unwrap();
        assert!(manager.get_profile("test").is_some());
        assert!(manager.get_profile("nonexistent").is_none());
        assert_eq!(manager.list_profiles().len(), 1);
    }
}
