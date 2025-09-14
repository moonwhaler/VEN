// Integration tests for Dolby Vision functionality
// This file demonstrates the complete Dolby Vision workflow

use crate::analysis::dolby_vision::{DolbyVisionDetector, DolbyVisionInfo, DolbyVisionProfile};
use crate::config::DolbyVisionConfig;
use crate::config::profiles::EncodingProfile;
use crate::config::types::RawProfile;
use crate::dolby_vision::{DoviTool, DoviToolConfig, RpuManager, RpuMetadata};
use std::collections::HashMap;
use std::path::PathBuf;

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_dolby_vision_profile_detection() {
        // Test Dolby Vision profile detection
        assert_eq!(DolbyVisionProfile::from_string("dvhe.05"), Some(DolbyVisionProfile::Profile5));
        assert_eq!(DolbyVisionProfile::from_string("dvhe.07"), Some(DolbyVisionProfile::Profile7));
        assert_eq!(DolbyVisionProfile::from_string("dvhe.08.06"), Some(DolbyVisionProfile::Profile81));
        assert_eq!(DolbyVisionProfile::from_string("8.1"), Some(DolbyVisionProfile::Profile81));
    }
    
    #[test]
    fn test_dolby_vision_config() {
        let config = DolbyVisionConfig::default();
        assert!(config.enabled);
        assert!(config.preserve_profile_7);
        assert_eq!(config.target_profile, "8.1");
        assert!(config.auto_profile_conversion);
        assert!(config.fallback_to_hdr10);
    }
    
    #[test]
    fn test_dolby_vision_detector_creation() {
        let config = DolbyVisionConfig::default();
        let detector = DolbyVisionDetector::new(config.clone());
        
        let dv_info = DolbyVisionInfo {
            profile: DolbyVisionProfile::Profile7,
            has_rpu: true,
            ..Default::default()
        };
        
        assert!(detector.should_preserve_dolby_vision(&dv_info));
        assert_eq!(detector.get_target_profile(DolbyVisionProfile::Profile7), 
                  DolbyVisionProfile::Profile81);
    }
    
    #[test]
    fn test_dovi_tool_configuration() {
        let config = DoviToolConfig::default();
        assert_eq!(config.path, "dovi_tool");
        assert_eq!(config.timeout_seconds, 300);
        
        let custom_config = DoviToolConfig {
            path: "/usr/local/bin/dovi_tool".to_string(),
            timeout_seconds: 600,
            extract_args: Some(vec!["--verbose".to_string()]),
            inject_args: Some(vec!["--force".to_string()]),
        };
        
        let tool = DoviTool::new(custom_config.clone());
        // Test that tool stores config correctly (internal state)
        // Since config is private, we can't directly test it, but constructor should work
        drop(tool); // Ensure it can be created and dropped
    }
    
    #[test]
    fn test_rpu_manager_creation() {
        let temp_dir = PathBuf::from("/tmp/rpu_test");
        let manager = RpuManager::new(temp_dir.clone(), None);
        
        // Test overhead estimation
        let dv_info_p7 = DolbyVisionInfo {
            profile: DolbyVisionProfile::Profile7,
            has_rpu: true,
            ..Default::default()
        };
        
        let dv_info_p81 = DolbyVisionInfo {
            profile: DolbyVisionProfile::Profile81,
            has_rpu: true,
            ..Default::default()
        };
        
        assert_eq!(manager.estimate_processing_overhead(&dv_info_p7), 1.8);
        assert_eq!(manager.estimate_processing_overhead(&dv_info_p81), 1.3);
    }
    
    #[test]
    fn test_profile_dolby_vision_compatibility() {
        // Create a test profile with 10-bit support
        let mut x265_params = HashMap::new();
        x265_params.insert("preset".to_string(), serde_yaml::Value::String("slow".to_string()));
        x265_params.insert("profile".to_string(), serde_yaml::Value::String("main10".to_string()));
        x265_params.insert("pix_fmt".to_string(), serde_yaml::Value::String("yuv420p10le".to_string()));
        
        let raw = RawProfile {
            title: "Dolby Vision Test Profile".to_string(),
            base_crf: 22.0,
            base_bitrate: 10000,
            hdr_bitrate: 13000,
            content_type: "film".to_string(),
            x265_params,
        };
        
        let profile = EncodingProfile::from_raw("dv_test".to_string(), raw).unwrap();
        assert!(profile.is_dolby_vision_compatible());
    }
    
    #[test]
    fn test_dolby_vision_x265_parameter_building() {
        // Create a mock profile
        let mut x265_params = HashMap::new();
        x265_params.insert("preset".to_string(), serde_yaml::Value::String("slow".to_string()));
        x265_params.insert("profile".to_string(), serde_yaml::Value::String("main10".to_string()));
        
        let raw = RawProfile {
            title: "DV Test Profile".to_string(),
            base_crf: 22.0,
            base_bitrate: 10000,
            hdr_bitrate: 13000,
            content_type: "film".to_string(),
            x265_params,
        };
        
        let profile = EncodingProfile::from_raw("dv_test".to_string(), raw).unwrap();
        
        // Create mock Dolby Vision info and RPU metadata
        let dv_info = DolbyVisionInfo {
            profile: DolbyVisionProfile::Profile81,
            has_rpu: true,
            ..Default::default()
        };
        
        let rpu_metadata = RpuMetadata {
            temp_file: PathBuf::from("/tmp/test.rpu"),
            profile: DolbyVisionProfile::Profile81,
            frame_count: Some(1000),
            extracted_successfully: true,
            file_size: Some(1024),
        };
        
        // Test parameter building with Dolby Vision
        let params_str = profile.build_x265_params_string_with_dolby_vision(
            None, // mode_specific_params
            Some(true), // is_hdr
            Some(&"bt2020nc".to_string()), // color_space
            Some(&"smpte2084".to_string()), // transfer_function
            Some(&"bt2020".to_string()), // color_primaries
            Some(&"G(0.17,0.797)B(0.131,0.046)R(0.708,0.292)WP(0.3127,0.329)L(1000,0.01)".to_string()), // master_display
            Some(&"1000".to_string()), // max_cll
            Some(&dv_info), // dv_info
            Some(&rpu_metadata), // rpu_metadata
        );
        
        // Verify Dolby Vision parameters are included with proper Level 5.1 High Tier VBV values
        assert!(params_str.contains("dolby-vision-rpu=/tmp/test.rpu"));
        assert!(params_str.contains("dolby-vision-profile=8.1"));
        assert!(params_str.contains("vbv-bufsize=160000")); // Updated from 20000 to proper DV values
        assert!(params_str.contains("vbv-maxrate=160000")); // Updated from 20000 to proper DV values
        assert!(params_str.contains("output-depth=10"));
        assert!(params_str.contains("colorprim=bt2020"));
        assert!(params_str.contains("transfer=smpte2084"));
        assert!(params_str.contains("colormatrix=bt2020nc"));
        
        // Also verify HDR parameters are included
        assert!(params_str.contains("master-display=G(0.17,0.797)B(0.131,0.046)R(0.708,0.292)WP(0.3127,0.329)L(1000,0.01)"));
        assert!(params_str.contains("max-cll=1000,400"));
    }
    
    #[test]
    fn test_dolby_vision_profile_conversion_logic() {
        let config = DolbyVisionConfig {
            target_profile: "8.2".to_string(), // Target 8.2 instead of default 8.1
            ..Default::default()
        };
        
        let detector = DolbyVisionDetector::new(config);
        
        // Test Profile 7 -> 8.2 conversion
        assert_eq!(detector.get_target_profile(DolbyVisionProfile::Profile7),
                  DolbyVisionProfile::Profile82);
                  
        // Test that other profiles remain unchanged
        assert_eq!(detector.get_target_profile(DolbyVisionProfile::Profile81),
                  DolbyVisionProfile::Profile81);
    }
    
    #[test]
    fn test_dolby_vision_fallback_scenarios() {
        // Test disabled Dolby Vision
        let disabled_config = DolbyVisionConfig {
            enabled: false,
            ..Default::default()
        };
        
        let detector = DolbyVisionDetector::new(disabled_config);
        
        let dv_info = DolbyVisionInfo {
            profile: DolbyVisionProfile::Profile7,
            has_rpu: true,
            ..Default::default()
        };
        
        // Should not preserve when disabled
        assert!(!detector.should_preserve_dolby_vision(&dv_info));
        
        // Test with require_dovi_tool but no tool available
        let _strict_config = DolbyVisionConfig {
            require_dovi_tool: true,
            ..Default::default()
        };
        
        // Create manager without dovi_tool
        let _manager = RpuManager::new(PathBuf::from("/tmp"), None);
        
        // The manager should handle the missing tool gracefully
        // (This is tested in the async test below)
    }
    
    #[tokio::test]
    async fn test_rpu_manager_without_dovi_tool() {
        let temp_dir = PathBuf::from("/tmp/test_rpu");
        let manager = RpuManager::new(temp_dir, None);
        
        let dv_info = DolbyVisionInfo {
            profile: DolbyVisionProfile::Profile81,
            has_rpu: true,
            ..Default::default()
        };
        
        // Should return error when no dovi_tool is configured
        let result = manager.extract_rpu("test.mkv", &dv_info).await;
        assert!(result.is_err());
        
        // Check that capability detection returns false
        let has_capability = manager.check_rpu_capability().await.unwrap();
        assert!(!has_capability);
    }
}

/// Demo function showing complete Dolby Vision workflow
/// This would be called from the main encoding pipeline
pub async fn demo_dolby_vision_workflow() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Dolby Vision Implementation Demo ===");
    
    // 1. Configuration
    let dv_config = DolbyVisionConfig::default();
    println!("✓ Dolby Vision configuration loaded");
    
    // 2. Detection (would be called with real ffprobe data)
    let detector = DolbyVisionDetector::new(dv_config.clone());
    println!("✓ Dolby Vision detector initialized");
    
    // 3. Mock detection result
    let dv_info = DolbyVisionInfo {
        profile: DolbyVisionProfile::Profile7,
        has_rpu: true,
        has_enhancement_layer: true,
        el_present: true,
        rpu_present: true,
        bl_compatible_id: Some(1),
        codec_profile: Some("dvhe.07.01".to_string()),
        spatial_resampling_filter_hint: None,
    };
    println!("✓ Detected Dolby Vision Profile 7 content");
    
    // 4. Profile conversion decision
    let target_profile = detector.get_target_profile(dv_info.profile);
    println!("✓ Target profile: {} -> {}", dv_info.profile.as_str(), target_profile.as_str());
    
    // 5. RPU Management (would use real dovi_tool)
    let temp_dir = PathBuf::from("/tmp");
    let dovi_config = DoviToolConfig::default();
    let dovi_tool = DoviTool::new(dovi_config);
    let rpu_manager = RpuManager::new(temp_dir, Some(dovi_tool));
    println!("✓ RPU manager initialized");
    
    // 6. Encoding profile with DV support
    let mut x265_params = HashMap::new();
    x265_params.insert("preset".to_string(), serde_yaml::Value::String("slow".to_string()));
    x265_params.insert("profile".to_string(), serde_yaml::Value::String("main10".to_string()));
    x265_params.insert("pix_fmt".to_string(), serde_yaml::Value::String("yuv420p10le".to_string()));
    
    let raw_profile = RawProfile {
        title: "Dolby Vision Movie Profile".to_string(),
        base_crf: 22.0,
        base_bitrate: 12000,
        hdr_bitrate: 16000,
        content_type: "film".to_string(),
        x265_params,
    };
    
    let profile = EncodingProfile::from_raw("dv_movie".to_string(), raw_profile)?;
    println!("✓ Dolby Vision-compatible encoding profile created");
    println!("  - Profile: {}", profile.title);
    println!("  - DV Compatible: {}", profile.is_dolby_vision_compatible());
    
    // 7. Mock RPU metadata (would be extracted from real content)
    let mock_rpu = RpuMetadata {
        temp_file: PathBuf::from("/tmp/extracted.rpu"),
        profile: target_profile,
        frame_count: Some(143892), // ~1 hour at 24fps
        extracted_successfully: true,
        file_size: Some(2048576), // 2MB RPU file
    };
    println!("✓ Mock RPU metadata created");
    println!("  - Frames: {:?}", mock_rpu.frame_count);
    println!("  - Size: {:?} bytes", mock_rpu.file_size);
    
    // 8. Build final x265 parameters with Dolby Vision
    let final_params = profile.build_x265_params_string_with_dolby_vision(
        None, // mode params
        Some(true), // is HDR
        Some(&"bt2020nc".to_string()),
        Some(&"smpte2084".to_string()),
        Some(&"bt2020".to_string()),
        Some(&"G(0.17,0.797)B(0.131,0.046)R(0.708,0.292)WP(0.3127,0.329)L(4000,0.005)".to_string()),
        Some(&"4000".to_string()),
        Some(&dv_info),
        Some(&mock_rpu),
    );
    
    println!("✓ Final x265 parameters with Dolby Vision support:");
    println!("  {}", final_params);
    
    // Verify that proper Level 5.1 High Tier VBV constraints are applied
    assert!(final_params.contains("vbv-bufsize=160000"));
    assert!(final_params.contains("vbv-maxrate=160000"));
    println!("✓ Verified Level 5.1 High Tier VBV constraints (160,000 kbps)");
    
    // 9. Processing overhead estimation
    let overhead = rpu_manager.estimate_processing_overhead(&dv_info);
    println!("✓ Estimated processing overhead: {}x", overhead);
    
    println!("=== Demo completed successfully ===");
    Ok(())
}

#[cfg(test)]
mod integration_tests {
    use super::*;
    
    #[tokio::test]
    async fn test_complete_dolby_vision_workflow() {
        // This test runs the complete demo workflow
        assert!(demo_dolby_vision_workflow().await.is_ok());
    }
}