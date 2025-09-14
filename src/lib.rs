pub mod analysis;
pub mod cli;
pub mod color;
pub mod config;
pub mod content_manager;
pub mod dolby_vision;
pub mod dolby_vision_integration_test;
pub mod encoding;
pub mod hdr;
pub mod hdr10plus;
pub mod progress;
pub mod stream;
pub mod utils;

pub use analysis::{ContentClassification, DolbyVisionInfo, DolbyVisionProfile, VideoAnalysis};
pub use color::ColorManager;
pub use config::{Config, ContentType, DolbyVisionConfig, EncodingProfile, UnifiedHdrConfig};
pub use content_manager::{
    ContentAnalysisResult, ContentEncodingApproach, EncodingAdjustments, UnifiedContentManager,
};
pub use dolby_vision::{DoviTool, DoviToolConfig, RpuManager, RpuMetadata};
pub use encoding::{EncodingMode, EncodingOptions};
pub use hdr::{ColorSpace, HdrFormat, HdrManager, HdrMetadata, TransferFunction};
pub use hdr10plus::{
    Hdr10PlusManager, Hdr10PlusMetadata, Hdr10PlusProcessingResult, Hdr10PlusToolConfig,
};
pub use stream::preservation::{StreamMapping, StreamPreservation};
pub use utils::{Error, FfmpegWrapper, Result};
