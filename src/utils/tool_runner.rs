use crate::utils::{Error, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::time::Duration;
use tokio::process::Command;
use tracing::{debug, error};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolConfig {
    pub path: String,
    pub timeout_seconds: u64,
    pub extract_args: Option<Vec<String>>,
    pub inject_args: Option<Vec<String>>,
}

impl Default for ToolConfig {
    fn default() -> Self {
        Self {
            path: "tool".to_string(),
            timeout_seconds: 300,
            extract_args: None,
            inject_args: None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ToolRunner {
    config: ToolConfig,
    timeout: Duration,
}

impl ToolRunner {
    pub fn new(config: ToolConfig) -> Self {
        let timeout = Duration::from_secs(config.timeout_seconds);
        Self { config, timeout }
    }

    pub async fn check_availability(
        &self,
        help_arg: &str,
        expected_subcommand: &str,
    ) -> Result<()> {
        debug!("Checking tool availability at: {}", self.config.path);

        let output = Command::new(&self.config.path)
            .arg(help_arg)
            .output()
            .await
            .map_err(|e| Error::Tool(format!("Failed to run tool: {}", e)))?;

        if !output.status.success() {
            return Err(Error::Tool(format!(
                "Tool check failed with exit code: {}",
                output.status
            )));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        if !stdout.contains(expected_subcommand) {
            return Err(Error::Tool(format!(
                "Tool appears to be missing required subcommand: {}",
                expected_subcommand
            )));
        }

        debug!("Tool is available and functional");
        Ok(())
    }

    pub async fn run(&self, args: &[String], output_file: Option<&Path>) -> Result<String> {
        self.run_with_logging(args, output_file, true).await
    }

    pub async fn run_silent(&self, args: &[String], output_file: Option<&Path>) -> Result<String> {
        self.run_with_logging(args, output_file, false).await
    }

    async fn run_with_logging(
        &self,
        args: &[String],
        output_file: Option<&Path>,
        log_errors: bool,
    ) -> Result<String> {
        let mut command = Command::new(&self.config.path);
        command.args(args);

        // Always capture stdout/stderr to prevent external tool messages from showing
        // without timestamps, which breaks log formatting
        command.stdout(std::process::Stdio::piped());
        command.stderr(std::process::Stdio::piped());

        debug!("Running: {} {}", self.config.path, args.join(" "));

        let child = command
            .spawn()
            .map_err(|e| Error::Tool(format!("Failed to spawn tool: {}", e)))?;

        let output = tokio::time::timeout(self.timeout, child.wait_with_output())
            .await
            .map_err(|_| {
                Error::Tool(format!(
                    "Tool timed out after {} seconds",
                    self.timeout.as_secs()
                ))
            })?
            .map_err(|e| Error::Tool(format!("Tool failed: {}", e)))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let stdout = String::from_utf8_lossy(&output.stdout);

            if log_errors {
                error!("Tool failed:");
                error!("Exit code: {}", output.status);
                error!("Stdout: {}", stdout);
                error!("Stderr: {}", stderr);
            } else {
                debug!("Tool failed (expected):");
                debug!("Exit code: {}", output.status);
                debug!("Stdout: {}", stdout);
                debug!("Stderr: {}", stderr);
            }

            return Err(Error::Tool(format!(
                "Tool failed with exit code {}: {}",
                output.status, stderr
            )));
        }

        // Log non-empty stdout/stderr from successful runs
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        if !stdout.is_empty() {
            // Log tool output line by line to maintain timestamp consistency
            for line in stdout.lines() {
                if !line.trim().is_empty() {
                    debug!("  {}", line);
                }
            }
        }

        if !stderr.is_empty() {
            // Some tools output progress to stderr even on success
            for line in stderr.lines() {
                if !line.trim().is_empty() {
                    debug!("  {}", line);
                }
            }
        }

        if let Some(file) = output_file {
            if !file.exists() {
                return Err(Error::Tool(
                    "Tool completed but output file not found".to_string(),
                ));
            }
        }

        Ok(stdout.to_string())
    }

    pub async fn run_with_custom_args<P: AsRef<Path>>(
        &self,
        base_args: &[String],
        custom_args: &Option<Vec<String>>,
        output_path: Option<P>,
    ) -> Result<String> {
        let mut args = base_args.to_vec();
        if let Some(ref custom) = custom_args {
            args.extend(custom.clone());
        }

        self.run(&args, output_path.as_ref().map(|p| p.as_ref()))
            .await
    }

    pub async fn run_with_custom_args_silent<P: AsRef<Path>>(
        &self,
        base_args: &[String],
        custom_args: &Option<Vec<String>>,
        output_path: Option<P>,
    ) -> Result<String> {
        let mut args = base_args.to_vec();
        if let Some(ref custom) = custom_args {
            args.extend(custom.clone());
        }

        self.run_silent(&args, output_path.as_ref().map(|p| p.as_ref()))
            .await
    }

    pub async fn get_version(&self) -> Result<String> {
        self.run(&["--version".to_string()], None).await
    }

    pub fn config(&self) -> &ToolConfig {
        &self.config
    }
}
