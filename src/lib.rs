pub mod analysis;
pub mod cli;
pub mod color;
pub mod config;
pub mod encoding;
pub mod hdr;
pub mod progress;
pub mod stream;
pub mod utils;

pub use analysis::{ContentClassification, VideoAnalysis};
pub use color::ColorManager;
pub use config::{Config, ContentType, EncodingProfile, UnifiedHdrConfig};
pub use encoding::{EncodingMode, EncodingOptions};
pub use hdr::{HdrFormat, HdrManager, HdrMetadata, ColorSpace, TransferFunction};
pub use stream::preservation::{StreamMapping, StreamPreservation};
pub use utils::{Error, FfmpegWrapper, Result};
