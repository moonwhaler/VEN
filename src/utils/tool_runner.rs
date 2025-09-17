use crate::utils::{Error, Result};
use std::path::Path;
use std::time::Duration;
use tokio::process::Command;
use tracing::{debug, error};

#[derive(Debug, Clone)]
pub struct ToolRunner {
    tool_path: String,
    timeout: Duration,
}

impl ToolRunner {
    pub fn new(tool_path: String, timeout_seconds: u64) -> Self {
        Self {
            tool_path,
            timeout: Duration::from_secs(timeout_seconds),
        }
    }

    pub async fn check_availability(&self, help_arg: &str, expected_subcommand: &str) -> Result<()> {
        debug!("Checking tool availability at: {}", self.tool_path);

        let output = Command::new(&self.tool_path)
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

    pub async fn run(
        &self,
        args: &[String],
        output_file: Option<&Path>,
    ) -> Result<String> {
        let mut command = Command::new(&self.tool_path);
        command.args(args);

        debug!("Running: {} {}", self.tool_path, args.join(" "));

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

            error!("Tool failed:");
            error!("Exit code: {}", output.status);
            error!("Stdout: {}", stdout);
            error!("Stderr: {}", stderr);

            return Err(Error::Tool(format!(
                "Tool failed with exit code {}: {}",
                output.status, stderr
            )));
        }

        if let Some(file) = output_file {
            if !file.exists() {
                return Err(Error::Tool(
                    "Tool completed but output file not found".to_string(),
                ));
            }
        }

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        debug!("Tool output: {}", stdout);

        Ok(stdout)
    }
}
