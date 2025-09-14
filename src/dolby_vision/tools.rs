use serde::{Deserialize, Serialize};
use std::path::Path;
use std::time::Duration;
use tokio::process::Command;
use tracing::{debug, info, error};

use crate::utils::{Result, Error};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DoviToolConfig {
    pub path: String,                      // Path to dovi_tool binary
    pub timeout_seconds: u64,              // Tool operation timeout
    pub extract_args: Option<Vec<String>>, // Custom extraction arguments
    pub inject_args: Option<Vec<String>>,  // Custom injection arguments
}

impl Default for DoviToolConfig {
    fn default() -> Self {
        Self {
            path: "dovi_tool".to_string(), // Assume it's in PATH
            timeout_seconds: 300, // 5 minutes default timeout
            extract_args: None,
            inject_args: None,
        }
    }
}

pub struct DoviTool {
    config: DoviToolConfig,
}

impl DoviTool {
    pub fn new(config: DoviToolConfig) -> Self {
        Self { config }
    }
    
    /// Check if dovi_tool is available and working
    pub async fn check_availability(&self) -> Result<()> {
        debug!("Checking dovi_tool availability at: {}", self.config.path);
        
        let output = Command::new(&self.config.path)
            .arg("--help")
            .output()
            .await
            .map_err(|e| Error::DolbyVision(format!("Failed to run dovi_tool: {}", e)))?;
        
        if !output.status.success() {
            return Err(Error::DolbyVision(
                format!("dovi_tool check failed with exit code: {}", output.status)
            ));
        }
        
        let stdout = String::from_utf8_lossy(&output.stdout);
        if !stdout.contains("extract-rpu") || !stdout.contains("inject-rpu") {
            return Err(Error::DolbyVision(
                "dovi_tool appears to be missing required extract-rpu and inject-rpu commands".to_string()
            ));
        }
        
        info!("dovi_tool is available and functional");
        Ok(())
    }
    
    /// Extract RPU metadata from input file
    pub async fn extract_rpu<P1: AsRef<Path>, P2: AsRef<Path>>(
        &self,
        input_path: P1,
        output_rpu: P2,
    ) -> Result<()> {
        let input_str = input_path.as_ref().to_string_lossy();
        let output_str = output_rpu.as_ref().to_string_lossy();
        
        info!("Extracting RPU: {} -> {}", input_str, output_str);
        
        // Build command arguments
        let mut args = vec![
            "extract-rpu".to_string(),
            input_str.to_string(),
            "-o".to_string(),
            output_str.to_string(),
        ];
        
        // Add custom extraction arguments if configured
        if let Some(ref custom_args) = self.config.extract_args {
            args.extend(custom_args.clone());
        }
        
        let mut command = Command::new(&self.config.path);
        command.args(&args);
        
        debug!("Running: {} {}", self.config.path, args.join(" "));
        
        // Execute with timeout
        let child = command.spawn()
            .map_err(|e| Error::DolbyVision(format!("Failed to spawn dovi_tool: {}", e)))?;
        
        let output = tokio::time::timeout(
            Duration::from_secs(self.config.timeout_seconds),
            child.wait_with_output()
        ).await
        .map_err(|_| Error::DolbyVision(format!(
            "dovi_tool extract-rpu timed out after {} seconds",
            self.config.timeout_seconds
        )))?
        .map_err(|e| Error::DolbyVision(format!("dovi_tool extract-rpu failed: {}", e)))?;
        
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let stdout = String::from_utf8_lossy(&output.stdout);
            
            error!("dovi_tool extract-rpu failed:");
            error!("Exit code: {}", output.status);
            error!("Stdout: {}", stdout);
            error!("Stderr: {}", stderr);
            
            return Err(Error::DolbyVision(format!(
                "RPU extraction failed with exit code {}: {}",
                output.status, stderr
            )));
        }
        
        let stdout = String::from_utf8_lossy(&output.stdout);
        debug!("dovi_tool extract-rpu output: {}", stdout);
        
        // Verify output file was created
        if !output_rpu.as_ref().exists() {
            return Err(Error::DolbyVision(
                "RPU extraction completed but output file not found".to_string()
            ));
        }
        
        info!("Successfully extracted RPU metadata");
        Ok(())
    }
    
    /// Inject RPU metadata into encoded HEVC file
    pub async fn inject_rpu<P1: AsRef<Path>, P2: AsRef<Path>, P3: AsRef<Path>>(
        &self,
        input_hevc: P1,
        rpu_file: P2,
        output_path: P3,
    ) -> Result<()> {
        let input_str = input_hevc.as_ref().to_string_lossy();
        let rpu_str = rpu_file.as_ref().to_string_lossy();
        let output_str = output_path.as_ref().to_string_lossy();
        
        info!("Injecting RPU: {} + {} -> {}", input_str, rpu_str, output_str);
        
        // Build command arguments
        let mut args = vec![
            "inject-rpu".to_string(),
            "-i".to_string(),
            input_str.to_string(),
            "--rpu-in".to_string(),
            rpu_str.to_string(),
            "-o".to_string(),
            output_str.to_string(),
        ];
        
        // Add custom injection arguments if configured
        if let Some(ref custom_args) = self.config.inject_args {
            args.extend(custom_args.clone());
        }
        
        let mut command = Command::new(&self.config.path);
        command.args(&args);
        
        debug!("Running: {} {}", self.config.path, args.join(" "));
        
        // Execute with timeout
        let child = command.spawn()
            .map_err(|e| Error::DolbyVision(format!("Failed to spawn dovi_tool: {}", e)))?;
        
        let output = tokio::time::timeout(
            Duration::from_secs(self.config.timeout_seconds),
            child.wait_with_output()
        ).await
        .map_err(|_| Error::DolbyVision(format!(
            "dovi_tool inject-rpu timed out after {} seconds",
            self.config.timeout_seconds
        )))?
        .map_err(|e| Error::DolbyVision(format!("dovi_tool inject-rpu failed: {}", e)))?;
        
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let stdout = String::from_utf8_lossy(&output.stdout);
            
            error!("dovi_tool inject-rpu failed:");
            error!("Exit code: {}", output.status);
            error!("Stdout: {}", stdout);
            error!("Stderr: {}", stderr);
            
            return Err(Error::DolbyVision(format!(
                "RPU injection failed with exit code {}: {}",
                output.status, stderr
            )));
        }
        
        let stdout = String::from_utf8_lossy(&output.stdout);
        debug!("dovi_tool inject-rpu output: {}", stdout);
        
        // Verify output file was created
        if !output_path.as_ref().exists() {
            return Err(Error::DolbyVision(
                "RPU injection completed but output file not found".to_string()
            ));
        }
        
        info!("Successfully injected RPU metadata");
        Ok(())
    }
    
    /// Convert Dolby Vision profile (e.g., Profile 7 to 8.1)
    pub async fn convert_profile<P1: AsRef<Path>, P2: AsRef<Path>>(
        &self,
        input_path: P1,
        output_path: P2,
        target_profile: &str,
    ) -> Result<()> {
        let input_str = input_path.as_ref().to_string_lossy();
        let output_str = output_path.as_ref().to_string_lossy();
        
        info!("Converting Dolby Vision profile to {}: {} -> {}", 
              target_profile, input_str, output_str);
        
        let args = vec![
            "convert".to_string(),
            input_str.to_string(),
            "-o".to_string(),
            output_str.to_string(),
            "--profile".to_string(),
            target_profile.to_string(),
        ];
        
        let mut command = Command::new(&self.config.path);
        command.args(&args);
        
        debug!("Running: {} {}", self.config.path, args.join(" "));
        
        let child = command.spawn()
            .map_err(|e| Error::DolbyVision(format!("Failed to spawn dovi_tool: {}", e)))?;
        
        let output = tokio::time::timeout(
            Duration::from_secs(self.config.timeout_seconds * 2), // Profile conversion can take longer
            child.wait_with_output()
        ).await
        .map_err(|_| Error::DolbyVision(format!(
            "dovi_tool profile conversion timed out after {} seconds",
            self.config.timeout_seconds * 2
        )))?
        .map_err(|e| Error::DolbyVision(format!("dovi_tool profile conversion failed: {}", e)))?;
        
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let stdout = String::from_utf8_lossy(&output.stdout);
            
            error!("dovi_tool profile conversion failed:");
            error!("Exit code: {}", output.status);
            error!("Stdout: {}", stdout);
            error!("Stderr: {}", stderr);
            
            return Err(Error::DolbyVision(format!(
                "Profile conversion failed with exit code {}: {}",
                output.status, stderr
            )));
        }
        
        info!("Successfully converted Dolby Vision profile to {}", target_profile);
        Ok(())
    }
    
    /// Get version information
    pub async fn get_version(&self) -> Result<String> {
        let output = Command::new(&self.config.path)
            .arg("--version")
            .output()
            .await
            .map_err(|e| Error::DolbyVision(format!("Failed to get dovi_tool version: {}", e)))?;
        
        if !output.status.success() {
            return Err(Error::DolbyVision(
                "Failed to get dovi_tool version information".to_string()
            ));
        }
        
        let version = String::from_utf8_lossy(&output.stdout).trim().to_string();
        debug!("dovi_tool version: {}", version);
        Ok(version)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_dovi_tool_config_default() {
        let config = DoviToolConfig::default();
        assert_eq!(config.path, "dovi_tool");
        assert_eq!(config.timeout_seconds, 300);
        assert_eq!(config.extract_args, None);
        assert_eq!(config.inject_args, None);
    }
    
    #[test]
    fn test_dovi_tool_creation() {
        let config = DoviToolConfig {
            path: "/usr/local/bin/dovi_tool".to_string(),
            timeout_seconds: 600,
            extract_args: Some(vec!["--verbose".to_string()]),
            inject_args: None,
        };
        
        let tool = DoviTool::new(config.clone());
        assert_eq!(tool.config, config);
    }
    
    // Note: The following tests require dovi_tool to be installed
    // They are ignored by default but can be enabled for integration testing
    
    #[ignore]
    #[tokio::test]
    async fn test_check_availability_integration() {
        let config = DoviToolConfig::default();
        let tool = DoviTool::new(config);
        
        // This will pass only if dovi_tool is actually installed
        match tool.check_availability().await {
            Ok(_) => println!("dovi_tool is available"),
            Err(e) => println!("dovi_tool not available: {}", e),
        }
    }
    
    #[ignore]
    #[tokio::test]
    async fn test_get_version_integration() {
        let config = DoviToolConfig::default();
        let tool = DoviTool::new(config);
        
        match tool.get_version().await {
            Ok(version) => println!("dovi_tool version: {}", version),
            Err(e) => println!("Failed to get version: {}", e),
        }
    }
}