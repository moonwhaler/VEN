pub mod hdr10;
pub mod hdr10_plus;
pub mod hlg;

pub use hdr10::*;
pub use hdr10_plus::*;
pub use hlg::*;

use crate::hdr::types::{HdrFormat, HdrMetadata};
use std::collections::HashMap;

/// Trait for format-specific HDR implementations
pub trait HdrFormatHandler {
    /// Get the HDR format this handler supports
    fn format(&self) -> HdrFormat;

    /// Build encoding parameters specific to this HDR format
    fn build_encoding_params(
        &self,
        metadata: &HdrMetadata,
        base_params: &HashMap<String, String>,
    ) -> HashMap<String, String>;

    /// Validate metadata for this HDR format
    fn validate_metadata(&self, metadata: &HdrMetadata) -> Result<(), String>;

    /// Get recommended encoding settings for this format
    fn get_encoding_recommendations(&self) -> EncodingRecommendations;
}

/// Format-specific encoding recommendations
#[derive(Debug, Clone)]
pub struct EncodingRecommendations {
    pub crf_adjustment: f32,
    pub bitrate_multiplier: f32,
    pub minimum_bit_depth: u8,
    pub recommended_preset: Option<String>,
    pub special_params: HashMap<String, String>,
}

/// HDR format registry for managing different HDR implementations
pub struct HdrFormatRegistry {
    handlers: HashMap<HdrFormat, Box<dyn HdrFormatHandler>>,
}

impl HdrFormatRegistry {
    pub fn new() -> Self {
        let mut registry = Self {
            handlers: HashMap::new(),
        };

        // Register default format handlers
        registry.register(Box::new(Hdr10Handler::new()));
        registry.register(Box::new(Hdr10PlusHandler::new()));
        registry.register(Box::new(HlgHandler::new()));

        registry
    }

    pub fn register(&mut self, handler: Box<dyn HdrFormatHandler>) {
        self.handlers.insert(handler.format(), handler);
    }

    pub fn get_handler(&self, format: HdrFormat) -> Option<&dyn HdrFormatHandler> {
        self.handlers.get(&format).map(|h| h.as_ref())
    }

    pub fn build_params_for_format(
        &self,
        format: HdrFormat,
        metadata: &HdrMetadata,
        base_params: &HashMap<String, String>,
    ) -> Result<HashMap<String, String>, String> {
        match self.get_handler(format) {
            Some(handler) => {
                // Validate metadata first
                handler.validate_metadata(metadata)?;
                // Build parameters
                Ok(handler.build_encoding_params(metadata, base_params))
            },
            None => Err(format!("No handler registered for HDR format {:?}", format)),
        }
    }

    pub fn get_recommendations(&self, format: HdrFormat) -> Option<EncodingRecommendations> {
        self.get_handler(format)
            .map(|handler| handler.get_encoding_recommendations())
    }

    pub fn supported_formats(&self) -> Vec<HdrFormat> {
        self.handlers.keys().cloned().collect()
    }
}

impl Default for HdrFormatRegistry {
    fn default() -> Self {
        Self::new()
    }
}