use crate::config::ContentType;
use crate::utils::Result;

#[derive(Debug, Clone)]
pub struct ContentClassification {
    pub content_type: ContentType,
    pub confidence: f32,
    pub method: String,
}

pub struct ContentAnalyzer;

impl ContentAnalyzer {
    pub fn new() -> Self {
        Self
    }

    pub async fn classify_content(
        &self,
        metadata: &crate::utils::ffmpeg::VideoMetadata,
    ) -> Result<ContentClassification> {
        let bitrate_per_pixel = metadata.bitrate.unwrap_or(0) as f64 / 
                               (metadata.width as f64 * metadata.height as f64);

        let content_type = if bitrate_per_pixel > 0.02 {
            ContentType::HeavyGrain
        } else if bitrate_per_pixel > 0.015 {
            ContentType::LightGrain
        } else if metadata.width >= 3840 || metadata.height >= 2160 {
            ContentType::Film
        } else {
            ContentType::Film
        };

        Ok(ContentClassification {
            content_type,
            confidence: 0.7, // Basic heuristic confidence
            method: "technical_analysis".to_string(),
        })
    }
}

impl Default for ContentAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}