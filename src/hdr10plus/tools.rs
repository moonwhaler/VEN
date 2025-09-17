use crate::utils::tool_runner::ToolRunner;
use crate::utils::Result;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tracing::{debug, info, warn};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Hdr10PlusToolConfig {
    pub path: String,
    pub timeout_seconds: u64,
    pub extract_args: Option<Vec<String>>,
    pub inject_args: Option<Vec<String>>,
}

impl Default for Hdr10PlusToolConfig {
    fn default() -> Self {
        Self {
            path: "hdr10plus_tool".to_string(),
            timeout_seconds: 300, // 5 minutes for processing
            extract_args: None,
            inject_args: None,
        }
    }
}

/// Wrapper for hdr10plus_tool external binary
pub struct Hdr10PlusTool {
    runner: ToolRunner,
    config: Hdr10PlusToolConfig,
}

impl Hdr10PlusTool {
    pub fn new(config: Hdr10PlusToolConfig) -> Self {
        let runner = ToolRunner::new(config.path.clone(), config.timeout_seconds);
        Self { runner, config }
    }

    /// Check if hdr10plus_tool is available
    pub async fn check_availability(&self) -> Result<bool> {
        match self.runner.check_availability("--help", "extract").await {
            Ok(_) => Ok(true),
            Err(_) => Ok(false),
        }
    }

    /// Extract HDR10+ metadata from video file to JSON
    pub async fn extract_metadata<P1: AsRef<Path>, P2: AsRef<Path>>(
        &self,
        input_video: P1,
        output_json: P2,
    ) -> Result<()> {
        let input_path = input_video.as_ref().to_string_lossy();
        let output_path = output_json.as_ref().to_string_lossy();

        info!(
            "Extracting HDR10+ metadata: {} -> {}",
            input_path, output_path
        );

        let mut args = vec![
            "extract".to_string(),
            input_path.to_string(),
            "-o".to_string(),
            output_path.to_string(),
        ];

        if let Some(ref custom_args) = self.config.extract_args {
            args.extend_from_slice(custom_args);
        }

        self.runner
            .run(&args, Some(output_json.as_ref()))
            .await
            .map(|_| ())
    }

    /// Inject HDR10+ metadata from JSON into video stream
    pub async fn inject_metadata<P1: AsRef<Path>, P2: AsRef<Path>, P3: AsRef<Path>>(
        &self,
        input_video: P1,
        metadata_json: P2,
        output_video: P3,
    ) -> Result<()> {
        let input_path = input_video.as_ref().to_string_lossy();
        let json_path = metadata_json.as_ref().to_string_lossy();
        let output_path = output_video.as_ref().to_string_lossy();

        info!(
            "Injecting HDR10+ metadata: {} + {} -> {}",
            input_path, json_path, output_path
        );

        let mut args = vec![
            "inject".to_string(),
            "-i".to_string(),
            input_path.to_string(),
            "-j".to_string(),
            json_path.to_string(),
            "-o".to_string(),
            output_path.to_string(),
        ];

        if let Some(ref custom_args) = self.config.inject_args {
            args.extend_from_slice(custom_args);
        }

        self.runner
            .run(&args, Some(output_video.as_ref()))
            .await
            .map(|_| ())
    }

    /// Remove HDR10+ metadata from video file
    pub async fn remove_metadata<P1: AsRef<Path>, P2: AsRef<Path>>(
        &self,
        input_video: P1,
        output_video: P2,
    ) -> Result<()> {
        let input_path = input_video.as_ref().to_string_lossy();
        let output_path = output_video.as_ref().to_string_lossy();

        info!(
            "Removing HDR10+ metadata: {} -> {}",
            input_path, output_path
        );

        let args = vec![
            "remove".to_string(),
            "-i".to_string(),
            input_path.to_string(),
            "-o".to_string(),
            output_path.to_string(),
        ];

        self.runner
            .run(&args, Some(output_video.as_ref()))
            .await
            .map(|_| ())
    }

    /// Generate a plot of HDR10+ brightness data
    pub async fn plot_metadata<P1: AsRef<Path>, P2: AsRef<Path>>(
        &self,
        metadata_json: P1,
        output_image: P2,
    ) -> Result<()> {
        let json_path = metadata_json.as_ref().to_string_lossy();
        let image_path = output_image.as_ref().to_string_lossy();

        info!("Plotting HDR10+ metadata: {} -> {}", json_path, image_path);

        let args = vec![
            "plot".to_string(),
            json_path.to_string(),
            "-o".to_string(),
            image_path.to_string(),
        ];

        self.runner
            .run(&args, Some(output_image.as_ref()))
            .await
            .map(|_| ())
    }

    /// Validate HDR10+ metadata JSON file
    pub async fn validate_metadata<P: AsRef<Path>>(&self, metadata_json: P) -> Result<bool> {
        let json_path = metadata_json.as_ref().to_string_lossy();

        debug!("Validating HDR10+ metadata: {}", json_path);

        // hdr10plus_tool doesn't have a separate validation command,
        // but we can try to plot it to verify it's valid
        let temp_plot = PathBuf::from("/tmp/hdr10plus_validation_plot.png");

        match self.plot_metadata(&metadata_json, &temp_plot).await {
            Ok(_) => {
                // Clean up temp file
                if temp_plot.exists() {
                    let _ = tokio::fs::remove_file(&temp_plot).await;
                }
                debug!("HDR10+ metadata validation: VALID");
                Ok(true)
            }
            Err(e) => {
                warn!("HDR10+ metadata validation failed: {}", e);
                Ok(false)
            }
        }
    }
}
