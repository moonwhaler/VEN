use super::types::{PreviewProfile, RawPreviewProfile};
use crate::utils::{Error, Result};
use std::collections::HashMap;

pub struct PreviewProfileManager {
    profiles: HashMap<String, PreviewProfile>,
}

impl PreviewProfileManager {
    pub fn new(raw_profiles: HashMap<String, RawPreviewProfile>) -> Result<Self> {
        let mut profiles = HashMap::new();

        for (name, raw) in raw_profiles {
            if raw.profiles.is_empty() {
                return Err(Error::validation(format!(
                    "Preview profile '{}' must specify at least one encoding profile",
                    name
                )));
            }

            let profile = PreviewProfile::from_raw(name.clone(), raw);
            profiles.insert(name, profile);
        }

        Ok(Self { profiles })
    }

    pub fn get_profile(&self, name: &str) -> Result<&PreviewProfile> {
        self.profiles.get(name).ok_or_else(|| {
            Error::validation(format!(
                "Preview profile '{}' not found. Available profiles: {}",
                name,
                self.list_profile_names().join(", ")
            ))
        })
    }

    pub fn list_profiles(&self) -> Vec<&PreviewProfile> {
        self.profiles.values().collect()
    }

    pub fn list_profile_names(&self) -> Vec<String> {
        let mut names: Vec<String> = self.profiles.keys().cloned().collect();
        names.sort();
        names
    }

    pub fn is_empty(&self) -> bool {
        self.profiles.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_preview_profile_manager() {
        let mut raw_profiles = HashMap::new();
        raw_profiles.insert(
            "anime_test".to_string(),
            RawPreviewProfile {
                title: "Anime Test".to_string(),
                profiles: vec!["anime".to_string(), "anime_new".to_string()],
            },
        );

        let manager = PreviewProfileManager::new(raw_profiles).unwrap();
        assert!(!manager.is_empty());
        assert_eq!(manager.list_profile_names(), vec!["anime_test"]);

        let profile = manager.get_profile("anime_test").unwrap();
        assert_eq!(profile.title, "Anime Test");
        assert_eq!(profile.profiles.len(), 2);
    }

    #[test]
    fn test_empty_profiles_validation() {
        let mut raw_profiles = HashMap::new();
        raw_profiles.insert(
            "invalid".to_string(),
            RawPreviewProfile {
                title: "Invalid".to_string(),
                profiles: vec![],
            },
        );

        let result = PreviewProfileManager::new(raw_profiles);
        assert!(result.is_err());
    }
}
