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
    #[must_use]
    pub fn new() -> Self {
        Self
    }

    /// Classify video content type based on bitrate per pixel heuristics
    ///
    /// # Errors
    ///
    /// Returns error if content classification fails
    pub async fn classify_content(
        &self,
        metadata: &crate::utils::ffmpeg::VideoMetadata,
    ) -> Result<ContentClassification> {
        let bitrate_per_pixel =
            f64::from(metadata.bitrate.unwrap_or(0)) / (f64::from(metadata.width) * f64::from(metadata.height));

        let content_type = if bitrate_per_pixel > 0.02 {
            ContentType::HeavyGrain
        } else if bitrate_per_pixel > 0.015 {
            ContentType::LightGrain
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
