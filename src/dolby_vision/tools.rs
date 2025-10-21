use crate::utils::{Result, ToolConfig, ToolRunner};
use std::path::Path;
use tracing::{debug, info};

pub type DoviToolConfig = ToolConfig;

pub struct DoviTool {
    tool: ToolRunner,
}

impl DoviTool {
    pub fn new(config: DoviToolConfig) -> Self {
        Self {
            tool: ToolRunner::new(config),
        }
    }

    pub async fn check_availability(&self) -> Result<()> {
        self.tool.check_availability("--help", "extract-rpu").await
    }

    pub async fn extract_rpu<P1: AsRef<Path>, P2: AsRef<Path>>(
        &self,
        input_path: P1,
        output_rpu: P2,
    ) -> Result<()> {
        let input_str = input_path.as_ref().to_string_lossy();
        let output_str = output_rpu.as_ref().to_string_lossy();

        info!("Extracting RPU: {} -> {}", input_str, output_str);
        debug!("Running dovi_tool (this may take a moment)...");

        let base_args = vec![
            "extract-rpu".to_string(),
            input_str.to_string(),
            "-o".to_string(),
            output_str.to_string(),
        ];

        self.tool
            .run_with_custom_args(
                &base_args,
                &self.tool.config().extract_args,
                Some(output_rpu),
            )
            .await
            .map(|_| ())
    }

    pub async fn inject_rpu<P1: AsRef<Path>, P2: AsRef<Path>, P3: AsRef<Path>>(
        &self,
        input_hevc: P1,
        rpu_file: P2,
        output_path: P3,
    ) -> Result<()> {
        let input_str = input_hevc.as_ref().to_string_lossy();
        let rpu_str = rpu_file.as_ref().to_string_lossy();
        let output_str = output_path.as_ref().to_string_lossy();

        info!(
            "Injecting RPU: {} + {} -> {}",
            input_str, rpu_str, output_str
        );
        debug!("Running dovi_tool (this may take a moment)...");

        let base_args = vec![
            "inject-rpu".to_string(),
            "-i".to_string(),
            input_str.to_string(),
            "--rpu-in".to_string(),
            rpu_str.to_string(),
            "-o".to_string(),
            output_str.to_string(),
        ];

        self.tool
            .run_with_custom_args(
                &base_args,
                &self.tool.config().inject_args,
                Some(output_path),
            )
            .await
            .map(|_| ())
    }

    pub async fn convert_profile<P1: AsRef<Path>, P2: AsRef<Path>>(
        &self,
        input_path: P1,
        output_path: P2,
        target_profile: &str,
    ) -> Result<()> {
        let input_str = input_path.as_ref().to_string_lossy();
        let output_str = output_path.as_ref().to_string_lossy();

        info!(
            "Converting Dolby Vision profile to {}: {} -> {}",
            target_profile, input_str, output_str
        );

        let args = vec![
            "convert".to_string(),
            input_str.to_string(),
            "-o".to_string(),
            output_str.to_string(),
            "--profile".to_string(),
            target_profile.to_string(),
        ];

        self.tool
            .run_with_custom_args(&args, &None, Some(output_path))
            .await
            .map(|_| ())
    }

    pub async fn get_version(&self) -> Result<String> {
        self.tool.get_version().await
    }
}
