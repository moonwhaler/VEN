use super::types::{AudioSelectionConfig, RawStreamSelectionProfile, StreamSelectionProfile, SubtitleSelectionConfig};
use crate::utils::{Error, Result};
use std::collections::HashMap;
use tracing::{debug, info};

pub struct StreamSelectionProfileManager {
    profiles: HashMap<String, StreamSelectionProfile>,
}

impl StreamSelectionProfileManager {
    pub fn new(raw_profiles: HashMap<String, RawStreamSelectionProfile>) -> Result<Self> {
        let mut profiles = HashMap::new();

        // Convert raw profiles to processed profiles
        for (name, raw_profile) in raw_profiles {
            let profile = StreamSelectionProfile::from_raw(name.clone(), raw_profile);
            profiles.insert(name.clone(), profile);
        }

        // Add built-in default profiles if no profiles are defined
        if profiles.is_empty() {
            profiles = Self::create_default_profiles();
        }

        info!("Loaded {} stream selection profiles", profiles.len());
        for (name, profile) in &profiles {
            debug!("Stream selection profile '{}': {}", name, profile.title);
        }

        Ok(Self { profiles })
    }

    pub fn get_profile(&self, name: &str) -> Result<&StreamSelectionProfile> {
        self.profiles.get(name).ok_or_else(|| {
            Error::validation(format!(
                "Stream selection profile '{}' not found. Available profiles: {}",
                name,
                self.list_profile_names().join(", ")
            ))
        })
    }

    pub fn list_profiles(&self) -> &HashMap<String, StreamSelectionProfile> {
        &self.profiles
    }

    pub fn list_profile_names(&self) -> Vec<String> {
        self.profiles.keys().cloned().collect()
    }

    pub fn has_profile(&self, name: &str) -> bool {
        self.profiles.contains_key(name)
    }

    pub fn get_default_profile_name(&self) -> String {
        if self.profiles.contains_key("default") {
            "default".to_string()
        } else {
            // Return the first available profile
            self.profiles.keys().next().cloned().unwrap_or_else(|| "none".to_string())
        }
    }

    fn create_default_profiles() -> HashMap<String, StreamSelectionProfile> {
        let mut profiles = HashMap::new();

        // Default: Copy everything (matches current behavior when stream_selection.enabled = false)
        profiles.insert(
            "default".to_string(),
            StreamSelectionProfile {
                name: "default".to_string(),
                title: "Default - Copy all streams".to_string(),
                audio: AudioSelectionConfig::default(),
                subtitle: SubtitleSelectionConfig::default(),
            },
        );

        // English only
        profiles.insert(
            "english_only".to_string(),
            StreamSelectionProfile {
                name: "english_only".to_string(),
                title: "English Only - Audio and subtitles".to_string(),
                audio: AudioSelectionConfig {
                    languages: Some(vec!["eng".to_string()]),
                    codecs: None,
                    dispositions: None,
                    title_patterns: None,
                    exclude_commentary: true,
                    max_streams: Some(2),
                },
                subtitle: SubtitleSelectionConfig {
                    languages: Some(vec!["eng".to_string()]),
                    codecs: None,
                    dispositions: None,
                    title_patterns: None,
                    exclude_commentary: true,
                    include_forced_only: false,
                    max_streams: Some(2),
                },
            },
        );

        // Multi-language (English and Japanese)
        profiles.insert(
            "multilang".to_string(),
            StreamSelectionProfile {
                name: "multilang".to_string(),
                title: "Multi-language - English and Japanese".to_string(),
                audio: AudioSelectionConfig {
                    languages: Some(vec!["eng".to_string(), "jpn".to_string()]),
                    codecs: Some(vec!["aac".to_string(), "ac3".to_string(), "dts".to_string()]),
                    dispositions: Some(vec!["default".to_string(), "original".to_string()]),
                    title_patterns: Some(vec!["(?i)^(?!.*(commentary|director|behind|making|bonus)).*$".to_string()]),
                    exclude_commentary: true,
                    max_streams: Some(3),
                },
                subtitle: SubtitleSelectionConfig {
                    languages: Some(vec!["eng".to_string(), "jpn".to_string()]),
                    codecs: Some(vec!["subrip".to_string(), "ass".to_string()]),
                    dispositions: Some(vec!["forced".to_string(), "default".to_string()]),
                    title_patterns: Some(vec!["(?i)^(?!.*(commentary|director|behind|making|bonus)).*$".to_string()]),
                    exclude_commentary: true,
                    include_forced_only: false,
                    max_streams: Some(4),
                },
            },
        );

        // Forced subtitles only
        profiles.insert(
            "forced_only".to_string(),
            StreamSelectionProfile {
                name: "forced_only".to_string(),
                title: "Forced Subtitles - All audio, forced subs only".to_string(),
                audio: AudioSelectionConfig {
                    languages: None,
                    codecs: None,
                    dispositions: None,
                    title_patterns: None,
                    exclude_commentary: true,
                    max_streams: None,
                },
                subtitle: SubtitleSelectionConfig {
                    languages: None,
                    codecs: None,
                    dispositions: None,
                    title_patterns: None,
                    exclude_commentary: true,
                    include_forced_only: true,
                    max_streams: Some(2),
                },
            },
        );

        // Minimal setup
        profiles.insert(
            "minimal".to_string(),
            StreamSelectionProfile {
                name: "minimal".to_string(),
                title: "Minimal - One audio, forced subs only".to_string(),
                audio: AudioSelectionConfig {
                    languages: Some(vec!["eng".to_string()]),
                    codecs: None,
                    dispositions: None,
                    title_patterns: None,
                    exclude_commentary: true,
                    max_streams: Some(1),
                },
                subtitle: SubtitleSelectionConfig {
                    languages: None,
                    codecs: None,
                    dispositions: None,
                    title_patterns: None,
                    exclude_commentary: true,
                    include_forced_only: true,
                    max_streams: Some(1),
                },
            },
        );

        profiles
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stream_profile_manager_creation() {
        let raw_profiles = HashMap::new();
        let manager = StreamSelectionProfileManager::new(raw_profiles).unwrap();

        // Should create default profiles
        assert!(!manager.profiles.is_empty());
        assert!(manager.has_profile("default"));
        assert!(manager.has_profile("english_only"));
        assert!(manager.has_profile("minimal"));
    }

    #[test]
    fn test_get_profile() {
        let manager = StreamSelectionProfileManager::new(HashMap::new()).unwrap();
        let profile = manager.get_profile("english_only").unwrap();

        assert_eq!(profile.name, "english_only");
        assert_eq!(profile.audio.languages, Some(vec!["eng".to_string()]));
        assert!(profile.audio.exclude_commentary);
    }

    #[test]
    fn test_invalid_profile() {
        let manager = StreamSelectionProfileManager::new(HashMap::new()).unwrap();
        let result = manager.get_profile("nonexistent");

        assert!(result.is_err());
    }

    #[test]
    fn test_multilang_profile_structure() {
        let manager = StreamSelectionProfileManager::new(HashMap::new()).unwrap();
        let profile = manager.get_profile("multilang").unwrap();

        assert_eq!(profile.audio.languages, Some(vec!["eng".to_string(), "jpn".to_string()]));
        assert_eq!(profile.audio.codecs, Some(vec!["aac".to_string(), "ac3".to_string(), "dts".to_string()]));
        assert_eq!(profile.subtitle.dispositions, Some(vec!["forced".to_string(), "default".to_string()]));
        assert!(profile.audio.title_patterns.is_some());
        assert_eq!(profile.audio.max_streams, Some(3));
        assert_eq!(profile.subtitle.max_streams, Some(4));
    }
}