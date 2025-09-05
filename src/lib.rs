pub mod cli;
pub mod config;
pub mod analysis;
pub mod encoding;
pub mod progress;
pub mod utils;
pub mod stream;
pub mod hardware;

pub use config::{Config, EncodingProfile, ContentType};
pub use encoding::{EncodingMode, EncodingOptions};
pub use analysis::{VideoAnalysis, ContentClassification};
pub use utils::{FfmpegWrapper, Error, Result};
pub use stream::preservation::{StreamPreservation, StreamMapping};
pub use hardware::cuda::{CudaAccelerator, HardwareAcceleration};