pub mod video;
pub mod content;
pub mod crop;

pub use video::VideoAnalysis;
pub use content::{ContentClassification, ContentAnalyzer};
pub use crop::{CropDetector, CropValues, CropAnalysisResult};
pub use crate::config::CropDetectionConfig;