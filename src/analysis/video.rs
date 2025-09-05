use serde::{Deserialize, Serialize};
use crate::config::ContentType;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VideoAnalysis {
    pub width: u32,
    pub height: u32,
    pub duration: f64,
    pub fps: f32,
    pub bitrate: Option<u32>,
    pub codec: Option<String>,
    pub is_hdr: bool,
    pub color_space: Option<String>,
    pub transfer_function: Option<String>,
    pub grain_level: u8,
    pub motion_level: u8,
    pub scene_changes: u32,
    pub complexity_score: f32,
    pub content_type: Option<ContentType>,
    pub crop_values: Option<String>,
}

impl VideoAnalysis {
    pub fn new(
        width: u32,
        height: u32,
        duration: f64,
        fps: f32,
    ) -> Self {
        Self {
            width,
            height,
            duration,
            fps,
            bitrate: None,
            codec: None,
            is_hdr: false,
            color_space: None,
            transfer_function: None,
            grain_level: 0,
            motion_level: 0,
            scene_changes: 0,
            complexity_score: 0.0,
            content_type: None,
            crop_values: None,
        }
    }

    pub fn is_4k(&self) -> bool {
        self.width >= 3840 || self.height >= 2160
    }

    pub fn is_1080p(&self) -> bool {
        !self.is_4k() && (self.width >= 1920 || self.height >= 1080)
    }

    pub fn resolution_category(&self) -> &'static str {
        if self.is_4k() {
            "4k"
        } else if self.is_1080p() {
            "1080p"
        } else {
            "720p"
        }
    }

    pub fn pixel_count(&self) -> u64 {
        self.width as u64 * self.height as u64
    }

    pub fn frame_count(&self) -> u64 {
        (self.duration * self.fps as f64) as u64
    }

    pub fn with_bitrate(mut self, bitrate: Option<u32>) -> Self {
        self.bitrate = bitrate;
        self
    }

    pub fn with_codec(mut self, codec: Option<String>) -> Self {
        self.codec = codec;
        self
    }

    pub fn with_hdr_info(mut self, is_hdr: bool, color_space: Option<String>, transfer_function: Option<String>) -> Self {
        self.is_hdr = is_hdr;
        self.color_space = color_space;
        self.transfer_function = transfer_function;
        self
    }

    pub fn with_grain_level(mut self, grain_level: u8) -> Self {
        self.grain_level = grain_level;
        self
    }

    pub fn with_motion_level(mut self, motion_level: u8) -> Self {
        self.motion_level = motion_level;
        self
    }

    pub fn with_scene_changes(mut self, scene_changes: u32) -> Self {
        self.scene_changes = scene_changes;
        self
    }

    pub fn with_complexity_score(mut self, complexity_score: f32) -> Self {
        self.complexity_score = complexity_score;
        self
    }

    pub fn with_content_type(mut self, content_type: Option<ContentType>) -> Self {
        self.content_type = content_type;
        self
    }

    pub fn with_crop_values(mut self, crop_values: Option<String>) -> Self {
        self.crop_values = crop_values;
        self
    }
}

impl Default for VideoAnalysis {
    fn default() -> Self {
        Self::new(1920, 1080, 3600.0, 24.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_video_analysis_resolution_detection() {
        let analysis_4k = VideoAnalysis::new(3840, 2160, 3600.0, 24.0);
        assert!(analysis_4k.is_4k());
        assert!(!analysis_4k.is_1080p());
        assert_eq!(analysis_4k.resolution_category(), "4k");

        let analysis_1080p = VideoAnalysis::new(1920, 1080, 3600.0, 24.0);
        assert!(!analysis_1080p.is_4k());
        assert!(analysis_1080p.is_1080p());
        assert_eq!(analysis_1080p.resolution_category(), "1080p");

        let analysis_720p = VideoAnalysis::new(1280, 720, 3600.0, 24.0);
        assert!(!analysis_720p.is_4k());
        assert!(!analysis_720p.is_1080p());
        assert_eq!(analysis_720p.resolution_category(), "720p");
    }

    #[test]
    fn test_video_analysis_calculations() {
        let analysis = VideoAnalysis::new(1920, 1080, 3600.0, 30.0);
        
        assert_eq!(analysis.pixel_count(), 2_073_600);
        assert_eq!(analysis.frame_count(), 108_000);
    }

    #[test]
    fn test_video_analysis_builder() {
        let analysis = VideoAnalysis::new(1920, 1080, 3600.0, 24.0)
            .with_bitrate(Some(5000))
            .with_codec(Some("h264".to_string()))
            .with_hdr_info(true, Some("bt2020".to_string()), Some("smpte2084".to_string()))
            .with_grain_level(45)
            .with_motion_level(25)
            .with_scene_changes(150)
            .with_complexity_score(75.5)
            .with_content_type(Some(ContentType::Film))
            .with_crop_values(Some("1920:800:0:140".to_string()));

        assert_eq!(analysis.bitrate, Some(5000));
        assert_eq!(analysis.codec, Some("h264".to_string()));
        assert!(analysis.is_hdr);
        assert_eq!(analysis.color_space, Some("bt2020".to_string()));
        assert_eq!(analysis.transfer_function, Some("smpte2084".to_string()));
        assert_eq!(analysis.grain_level, 45);
        assert_eq!(analysis.motion_level, 25);
        assert_eq!(analysis.scene_changes, 150);
        assert_eq!(analysis.complexity_score, 75.5);
        assert_eq!(analysis.content_type, Some(ContentType::Film));
        assert_eq!(analysis.crop_values, Some("1920:800:0:140".to_string()));
    }
}