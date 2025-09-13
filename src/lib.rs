pub mod analysis;
pub mod cli;
pub mod config;
pub mod encoding;
pub mod progress;
pub mod stream;
pub mod utils;

pub use analysis::{ContentClassification, VideoAnalysis};
pub use config::{Config, ContentType, EncodingProfile};
pub use encoding::{EncodingMode, EncodingOptions};
pub use stream::preservation::{StreamMapping, StreamPreservation};
pub use utils::{Error, FfmpegWrapper, Result};
