use crate::utils::{Result, ToolConfig, ToolRunner};
use std::path::{Path, PathBuf};
use tracing::{debug, info, warn};

pub type Hdr10PlusToolConfig = ToolConfig;

pub struct Hdr10PlusTool {
    tool: ToolRunner,
}

impl Hdr10PlusTool {
    pub fn new(config: Hdr10PlusToolConfig) -> Self {
        Self {
            tool: ToolRunner::new(config),
        }
    }

    pub async fn check_availability(&self) -> Result<bool> {
        match self.tool.check_availability("--help", "extract").await {
            Ok(_) => Ok(true),
            Err(_) => Ok(false),
        }
    }

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
        info!("Running hdr10plus_tool (this may take a moment)...");

        let base_args = vec![
            "extract".to_string(),
            input_path.to_string(),
            "-o".to_string(),
            output_path.to_string(),
        ];

        // Use silent method to avoid scary ERROR logs for expected failures
        match self
            .tool
            .run_with_custom_args_silent(
                &base_args,
                &self.tool.config().extract_args,
                Some(output_json),
            )
            .await
        {
            Ok(_) => Ok(()),
            Err(e) => {
                // Check if this is the expected "no dynamic metadata" case
                let error_message = e.to_string();
                if error_message.contains("Tool failed with exit code exit status: 1") {
                    // This is expected when no HDR10+ metadata exists - return a custom error
                    // that can be handled more gracefully by the caller
                    Err(crate::utils::Error::Tool(
                        "File doesn't contain dynamic metadata".to_string(),
                    ))
                } else {
                    Err(e)
                }
            }
        }
    }

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
        info!("Running hdr10plus_tool (this may take a moment)...");

        let base_args = vec![
            "inject".to_string(),
            "-i".to_string(),
            input_path.to_string(),
            "-j".to_string(),
            json_path.to_string(),
            "-o".to_string(),
            output_path.to_string(),
        ];

        self.tool
            .run_with_custom_args(
                &base_args,
                &self.tool.config().inject_args,
                Some(output_video),
            )
            .await
            .map(|_| ())
    }

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

        self.tool
            .run_with_custom_args(&args, &None, Some(output_video))
            .await
            .map(|_| ())
    }

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

        self.tool
            .run_with_custom_args(&args, &None, Some(output_image))
            .await
            .map(|_| ())
    }

    pub async fn validate_metadata<P: AsRef<Path>>(&self, metadata_json: P) -> Result<bool> {
        let json_path = metadata_json.as_ref().to_string_lossy();

        debug!("Validating HDR10+ metadata: {}", json_path);

        let temp_plot = PathBuf::from("/tmp/hdr10plus_validation_plot.png");

        match self.plot_metadata(&metadata_json, &temp_plot).await {
            Ok(_) => {
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
