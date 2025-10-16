//! Modular logging system for FFmpeg encoder
//!
//! This module provides both console and file logging capabilities:
//! - Console logging with clean formatting and hierarchical output
//! - Detailed file logging for encoding operations
//! - Helper functions for common logging operations

// Internal modules
mod file_logger;
mod formatter;
mod helpers;
mod text_utils;

// Re-export public types and functions for backward compatibility
pub use file_logger::FileLogger;
pub use helpers::{
    log_analysis_result, log_crop_detection, log_encoding_complete, log_encoding_start,
    log_profile_selection,
};

use tracing::Level;
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

use formatter::CleanFormatter;

/// Sets up the logging system with the specified configuration
///
/// # Arguments
/// * `level` - Log level (trace, debug, info, warn, error)
/// * `show_timestamps` - Whether to show timestamps in console output
/// * `colored` - Whether to use colored output in console
///
/// # Examples
/// ```no_run
/// use ffmpeg_autoencoder_rust::utils::logging::setup_logging;
///
/// setup_logging("info", false, true).expect("Failed to setup logging");
/// ```
pub fn setup_logging(
    level: &str,
    show_timestamps: bool,
    colored: bool,
) -> crate::utils::Result<()> {
    let level = match level.to_lowercase().as_str() {
        "trace" => Level::TRACE,
        "debug" => Level::DEBUG,
        "info" => Level::INFO,
        "warn" => Level::WARN,
        "error" => Level::ERROR,
        _ => Level::INFO,
    };

    let env_filter = EnvFilter::builder()
        .with_default_directive(level.into())
        .from_env_lossy();

    // Use our clean formatter for better console output
    let formatter = CleanFormatter::new(show_timestamps, colored);
    let fmt_layer = fmt::layer()
        .with_target(false)
        .with_level(false) // We handle level formatting in our custom formatter
        .event_format(formatter);

    tracing_subscriber::registry()
        .with(env_filter)
        .with(fmt_layer)
        .init();

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_module_structure() {
        // Test that all public exports are accessible
        // This is a compile-time test more than a runtime test
        let _functions = (
            log_encoding_start,
            log_encoding_complete,
            log_analysis_result,
            log_crop_detection,
            log_profile_selection,
        );
    }
}
