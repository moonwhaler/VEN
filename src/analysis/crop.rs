use crate::utils::Result;
use crate::config::CropDetectionConfig;
use std::path::Path;
use tokio::process::Command;
use regex::Regex;
use once_cell::sync::Lazy;
use std::collections::HashMap;
use tracing::{info, debug};

static CROP_REGEX: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"crop=(\d+):(\d+):(\d+):(\d+)").unwrap()
});

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CropValues {
    pub width: u32,
    pub height: u32,
    pub x: u32,
    pub y: u32,
}

impl CropValues {
    pub fn new(width: u32, height: u32, x: u32, y: u32) -> Self {
        Self { width, height, x, y }
    }
    
    pub fn to_ffmpeg_string(&self) -> String {
        format!("{}:{}:{}:{}", self.width, self.height, self.x, self.y)
    }
    
    pub fn calculate_pixel_change(&self, original_width: u32, original_height: u32) -> f32 {
        let original_pixels = (original_width * original_height) as f32;
        let cropped_pixels = (self.width * self.height) as f32;
        
        ((original_pixels - cropped_pixels) / original_pixels) * 100.0
    }
    
    pub fn is_significant_crop(&self, original_width: u32, original_height: u32, min_change_percent: f32) -> bool {
        self.calculate_pixel_change(original_width, original_height) >= min_change_percent
    }
}

impl CropDetectionConfig {
    /// Generate evenly distributed sample timestamps across video duration
    pub fn get_sample_timestamps(&self, video_duration: f64) -> Vec<f64> {
        if self.sample_count == 0 {
            return vec![];
        }
        
        if self.sample_count == 1 {
            // Single sample at middle
            return vec![video_duration / 2.0];
        }
        
        // Generate evenly distributed timestamps
        let mut timestamps = Vec::new();
        
        // For multiple samples, distribute evenly across the video
        // Use smart margins based on video duration
        let margin_seconds = if video_duration < 60.0 {
            // For short videos, use smaller margins
            (video_duration * 0.1).max(1.0) // 10% margin, minimum 1 second
        } else {
            30.0 // 30 seconds margin for longer videos
        };
        
        let effective_duration = (video_duration - 2.0 * margin_seconds).max(video_duration * 0.5);
        let start_time = margin_seconds;
        
        for i in 0..self.sample_count {
            let ratio = i as f64 / (self.sample_count - 1) as f64; // Evenly distributed from 0.0 to 1.0
            let timestamp = start_time + (ratio * effective_duration);
            timestamps.push(timestamp);
        }
        
        timestamps
    }
}


#[derive(Debug, Clone)]
pub struct CropAnalysisResult {
    pub crop_values: Option<CropValues>,
    pub detection_method: String,
    pub confidence: f32,
    pub pixel_change_percent: f32,
    pub sample_results: Vec<CropSampleResult>,
}

#[derive(Debug, Clone)]
pub struct CropSampleResult {
    pub sample_point: String,
    pub timestamp: f64,
    pub crop_values: Option<CropValues>,
    pub raw_output: String,
}

pub struct CropDetector {
    config: CropDetectionConfig,
}

impl CropDetector {
    pub fn new(config: CropDetectionConfig) -> Self {
        Self { config }
    }
    
    pub async fn detect_crop_values<P: AsRef<Path>>(
        &self,
        input_path: P,
        duration: f64,
        width: u32,
        height: u32,
        is_hdr: bool,
    ) -> Result<CropAnalysisResult> {
        if !self.config.enabled {
            return Ok(CropAnalysisResult {
                crop_values: None,
                detection_method: "disabled".to_string(),
                confidence: 0.0,
                pixel_change_percent: 0.0,
                sample_results: vec![],
            });
        }
        
        let sample_timestamps = self.config.get_sample_timestamps(duration);
        info!("Starting crop detection analysis with {} sample points", sample_timestamps.len());
        
        // Multi-temporal sampling for crop detection
        let mut sample_results = Vec::new();
        
        for timestamp in &sample_timestamps {
            let sample_result = self.detect_crop_at_timestamp(
                input_path.as_ref(), 
                *timestamp, 
                is_hdr
            ).await?;
            
            sample_results.push(sample_result);
        }
        
        // Frequency analysis to find most common crop values
        let crop_analysis = self.analyze_crop_frequency(&sample_results, width, height);
        
        info!("Crop detection completed: {:?}", crop_analysis.crop_values);
        Ok(crop_analysis)
    }
    
    
    async fn detect_crop_at_timestamp<P: AsRef<Path>>(
        &self,
        input_path: P,
        timestamp: f64,
        is_hdr: bool,
    ) -> Result<CropSampleResult> {
        let input_path_str = input_path.as_ref().to_string_lossy();
        
        // Choose crop detection limit based on HDR vs SDR
        let crop_limit = if is_hdr {
            self.config.hdr_crop_limit
        } else {
            self.config.sdr_crop_limit
        };
        
        debug!("Detecting crop at timestamp {:.2}s with limit {} (HDR: {})", 
               timestamp, crop_limit, is_hdr);
        
        let output = Command::new("ffmpeg")
            .args([
                "-ss", &timestamp.to_string(),
                "-i", &input_path_str,
                "-t", "2", // Analyze 2 seconds for better accuracy
                "-vf", &format!("cropdetect=limit={}:round=2", crop_limit),
                "-f", "null",
                "-"
            ])
            .output()
            .await?;
        
        let stderr = String::from_utf8_lossy(&output.stderr);
        let crop_values = self.extract_crop_from_output(&stderr);
        
        Ok(CropSampleResult {
            sample_point: format!("{:.1}s", timestamp),
            timestamp,
            crop_values,
            raw_output: stderr.to_string(),
        })
    }
    
    fn extract_crop_from_output(&self, output: &str) -> Option<CropValues> {
        // Find the last (most recent) crop detection line
        let mut best_crop: Option<CropValues> = None;
        
        for line in output.lines() {
            if let Some(captures) = CROP_REGEX.captures(line) {
                let width: u32 = captures[1].parse().ok()?;
                let height: u32 = captures[2].parse().ok()?;
                let x: u32 = captures[3].parse().ok()?;
                let y: u32 = captures[4].parse().ok()?;
                
                best_crop = Some(CropValues::new(width, height, x, y));
            }
        }
        
        best_crop
    }
    
    fn analyze_crop_frequency(
        &self,
        sample_results: &[CropSampleResult],
        original_width: u32,
        original_height: u32,
    ) -> CropAnalysisResult {
        let mut crop_frequency: HashMap<CropValues, u32> = HashMap::new();
        let mut valid_samples = 0;
        
        // Count frequency of each unique crop value
        for sample in sample_results {
            if let Some(crop_values) = &sample.crop_values {
                *crop_frequency.entry(crop_values.clone()).or_insert(0) += 1;
                valid_samples += 1;
            }
        }
        
        if crop_frequency.is_empty() {
            return CropAnalysisResult {
                crop_values: None,
                detection_method: "no_crops_detected".to_string(),
                confidence: 0.0,
                pixel_change_percent: 0.0,
                sample_results: sample_results.to_vec(),
            };
        }
        
        // Find the most frequent crop value
        let (most_common_crop, frequency) = crop_frequency.iter()
            .max_by_key(|(_, &count)| count)
            .map(|(crop, &count)| (crop.clone(), count))
            .unwrap();
        
        // Calculate confidence based on frequency and significance
        let confidence = (frequency as f32 / valid_samples as f32) * 100.0;
        let pixel_change = most_common_crop.calculate_pixel_change(original_width, original_height);
        
        // Check if crop is significant enough to apply
        let should_apply_crop = most_common_crop.is_significant_crop(
            original_width, 
            original_height, 
            self.config.min_pixel_change_percent
        );
        
        let detection_method = if should_apply_crop {
            format!("frequency_analysis_{}%_agreement", (confidence as u32))
        } else {
            "insufficient_change".to_string()
        };
        
        debug!("Crop frequency analysis: {:?} appears {}% of the time, {:.2}% pixel change",
               most_common_crop, confidence, pixel_change);
        
        CropAnalysisResult {
            crop_values: if should_apply_crop { Some(most_common_crop) } else { None },
            detection_method,
            confidence,
            pixel_change_percent: pixel_change,
            sample_results: sample_results.to_vec(),
        }
    }
    
    pub async fn validate_crop_detection(
        &self,
        input_path: &Path,
        crop_values: &CropValues,
        duration: f64,
    ) -> Result<bool> {
        // Additional validation by testing crop at random points
        let validation_points = vec![
            duration * 0.2,
            duration * 0.5,
            duration * 0.8,
        ];
        
        let mut validation_results = Vec::new();
        
        for timestamp in validation_points {
            let result = self.detect_crop_at_timestamp(input_path, timestamp, false).await?;
            validation_results.push(result);
        }
        
        // Check if detected crop matches expected values (with some tolerance)
        let matching_results = validation_results.iter()
            .filter(|result| {
                if let Some(detected_crop) = &result.crop_values {
                    self.crops_match_with_tolerance(crop_values, detected_crop, 4)
                } else {
                    false
                }
            })
            .count();
        
        let validation_confidence = matching_results as f32 / validation_results.len() as f32;
        let is_valid = validation_confidence >= 0.5; // Require 50% agreement
        
        debug!("Crop validation: {}/{} samples match, confidence: {:.1}%",
               matching_results, validation_results.len(), validation_confidence * 100.0);
        
        Ok(is_valid)
    }
    
    fn crops_match_with_tolerance(&self, crop1: &CropValues, crop2: &CropValues, tolerance: u32) -> bool {
        let width_diff = (crop1.width as i32 - crop2.width as i32).unsigned_abs();
        let height_diff = (crop1.height as i32 - crop2.height as i32).unsigned_abs();
        let x_diff = (crop1.x as i32 - crop2.x as i32).unsigned_abs();
        let y_diff = (crop1.y as i32 - crop2.y as i32).unsigned_abs();
        
        width_diff <= tolerance && height_diff <= tolerance && 
        x_diff <= tolerance && y_diff <= tolerance
    }
    
    pub fn parse_sample_point(&self, sample_point: &str, duration: f64) -> Result<f64> {
        match sample_point {
            "middle" => Ok(duration / 2.0),
            point if point.ends_with('s') => {
                let time_str = &point[..point.len() - 1];
                if let Some(negative_time) = time_str.strip_prefix('-') {
                    let offset: f64 = negative_time.parse()
                        .map_err(|_| crate::utils::Error::validation("Invalid negative time format"))?;
                    Ok(duration - offset)
                } else {
                    time_str.parse()
                        .map_err(|_| crate::utils::Error::validation("Invalid time format"))
                }
            }
            point if point.ends_with('%') => {
                let percent_str = &point[..point.len() - 1];
                let percent: f64 = percent_str.parse()
                    .map_err(|_| crate::utils::Error::validation("Invalid percentage format"))?;
                Ok(duration * (percent / 100.0))
            }
            point => {
                // Try parsing as raw seconds
                point.parse()
                    .map_err(|_| crate::utils::Error::validation("Invalid sample point format"))
            }
        }
    }
}

impl Default for CropDetector {
    fn default() -> Self {
        Self::new(CropDetectionConfig::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_crop_values_creation() {
        let crop = CropValues::new(1920, 800, 0, 140);
        assert_eq!(crop.to_ffmpeg_string(), "1920:800:0:140");
    }

    #[test]
    fn test_pixel_change_calculation() {
        let crop = CropValues::new(1920, 800, 0, 140);
        let change = crop.calculate_pixel_change(1920, 1080);
        
        // Original: 1920 * 1080 = 2,073,600 pixels
        // Cropped: 1920 * 800 = 1,536,000 pixels
        // Change: (2,073,600 - 1,536,000) / 2,073,600 * 100 = ~25.93%
        assert!((change - 25.93).abs() < 0.1);
    }

    #[test]
    fn test_sample_point_parsing() {
        let detector = CropDetector::default();
        let duration = 3600.0; // 1 hour
        
        assert_eq!(detector.parse_sample_point("middle", duration).unwrap(), 1800.0);
        assert_eq!(detector.parse_sample_point("60s", duration).unwrap(), 60.0);
        assert_eq!(detector.parse_sample_point("-60s", duration).unwrap(), 3540.0);
        assert_eq!(detector.parse_sample_point("300", duration).unwrap(), 300.0);
    }

    #[test]
    fn test_crop_regex() {
        let output = "[Parsed_cropdetect_0 @ 0x7f8b8c000940] crop=1920:800:0:140";
        let captures = CROP_REGEX.captures(output).unwrap();
        
        assert_eq!(captures[1].parse::<u32>().unwrap(), 1920);
        assert_eq!(captures[2].parse::<u32>().unwrap(), 800);
        assert_eq!(captures[3].parse::<u32>().unwrap(), 0);
        assert_eq!(captures[4].parse::<u32>().unwrap(), 140);
    }

    #[test]
    fn test_crops_match_tolerance() {
        let detector = CropDetector::default();
        let crop1 = CropValues::new(1920, 800, 0, 140);
        let crop2 = CropValues::new(1918, 802, 2, 138); // Within 4 pixel tolerance
        let crop3 = CropValues::new(1900, 780, 10, 150); // Outside tolerance
        
        assert!(detector.crops_match_with_tolerance(&crop1, &crop2, 4));
        assert!(!detector.crops_match_with_tolerance(&crop1, &crop3, 4));
    }

    #[test]
    fn test_dynamic_sample_timestamps() {
        let mut config = CropDetectionConfig::default();
        config.sample_count = 5;
        
        // Test with 120 second video (2 minutes)
        let timestamps = config.get_sample_timestamps(120.0);
        assert_eq!(timestamps.len(), 5);
        
        // Should generate evenly distributed timestamps with margins
        // For 120s video with 30s margins: effective duration = 60s
        // Timestamps should be at: 30.0, 45.0, 60.0, 75.0, 90.0
        assert_eq!(timestamps[0], 30.0);
        assert_eq!(timestamps[1], 45.0); 
        assert_eq!(timestamps[2], 60.0);
        assert_eq!(timestamps[3], 75.0);
        assert_eq!(timestamps[4], 90.0);
    }

    #[test]
    fn test_single_sample_timestamp() {
        let mut config = CropDetectionConfig::default();
        config.sample_count = 1;
        
        let timestamps = config.get_sample_timestamps(120.0);
        assert_eq!(timestamps.len(), 1);
        assert_eq!(timestamps[0], 60.0); // Middle of 120s video
    }

    #[test]
    fn test_default_sample_count() {
        let config = CropDetectionConfig::default(); // sample_count = 3
        
        let timestamps = config.get_sample_timestamps(120.0);
        assert_eq!(timestamps.len(), 3);
        // Should be evenly distributed: 30.0, 60.0, 90.0
        assert_eq!(timestamps[0], 30.0);
        assert_eq!(timestamps[1], 60.0);
        assert_eq!(timestamps[2], 90.0);
    }

    #[test]
    fn test_zero_sample_count() {
        let mut config = CropDetectionConfig::default();
        config.sample_count = 0;
        
        let timestamps = config.get_sample_timestamps(120.0);
        assert_eq!(timestamps.len(), 0);
    }
}