use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tokio::fs;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use crate::analysis::dolby_vision::{DolbyVisionInfo, DolbyVisionProfile};
use crate::dolby_vision::tools::DoviTool;
use crate::utils::{Error, Result};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RpuMetadata {
    pub temp_file: PathBuf,
    pub profile: DolbyVisionProfile,
    pub frame_count: Option<u64>,
    pub extracted_successfully: bool,
    pub file_size: Option<u64>,
}

impl RpuMetadata {
    pub fn new(temp_file: PathBuf, profile: DolbyVisionProfile) -> Self {
        Self {
            temp_file,
            profile,
            frame_count: None,
            extracted_successfully: false,
            file_size: None,
        }
    }

    pub async fn validate(&mut self) -> Result<()> {
        if !self.temp_file.exists() {
            return Err(Error::Io(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("RPU file not found: {}", self.temp_file.display()),
            )));
        }

        let metadata = fs::metadata(&self.temp_file).await?;
        self.file_size = Some(metadata.len());

        if metadata.len() == 0 {
            return Err(Error::DolbyVision(
                "RPU file is empty - extraction may have failed".to_string(),
            ));
        }

        self.extracted_successfully = true;
        debug!(
            "Validated RPU file: {} ({} bytes)",
            self.temp_file.display(),
            metadata.len()
        );
        Ok(())
    }
}

pub struct RpuManager {
    temp_dir: PathBuf,
    dovi_tool: Option<DoviTool>,
}

impl RpuManager {
    pub fn new(temp_dir: PathBuf, dovi_tool: Option<DoviTool>) -> Self {
        Self {
            temp_dir,
            dovi_tool,
        }
    }

    pub async fn ensure_temp_dir(&self) -> Result<()> {
        if !self.temp_dir.exists() {
            fs::create_dir_all(&self.temp_dir).await?;
            debug!("Created temporary directory: {}", self.temp_dir.display());
        }
        Ok(())
    }

    /// Extract RPU metadata from input file
    pub async fn extract_rpu<P: AsRef<Path>>(
        &self,
        input_path: P,
        dv_info: &DolbyVisionInfo,
    ) -> Result<Option<RpuMetadata>> {
        if !dv_info.needs_rpu_processing() {
            debug!("No RPU processing needed for this content");
            return Ok(None);
        }

        let dovi_tool = self.dovi_tool.as_ref().ok_or_else(|| {
            Error::DolbyVision(
                "dovi_tool not configured but required for RPU extraction".to_string(),
            )
        })?;

        // Generate RPU file alongside the source video
        let input_path_ref = input_path.as_ref();
        let rpu_filename = if let Some(stem) = input_path_ref.file_stem() {
            format!("{}_rpu_{}.bin", stem.to_string_lossy(), Uuid::new_v4())
        } else {
            format!("rpu_{}.bin", Uuid::new_v4())
        };

        let rpu_path = if let Some(parent) = input_path_ref.parent() {
            parent.join(rpu_filename)
        } else {
            PathBuf::from(rpu_filename)
        };

        info!(
            "Extracting RPU metadata from: {}",
            input_path.as_ref().display()
        );

        // Extract RPU using dovi_tool
        match dovi_tool.extract_rpu(input_path, rpu_path.clone()).await {
            Ok(_) => {
                let mut rpu_metadata = RpuMetadata::new(rpu_path, dv_info.profile);

                match rpu_metadata.validate().await {
                    Ok(_) => {
                        info!(
                            "Successfully extracted RPU metadata for Profile {}",
                            dv_info.profile.as_str()
                        );
                        Ok(Some(rpu_metadata))
                    }
                    Err(e) => {
                        error!("RPU validation failed: {}", e);
                        self.cleanup_rpu(&rpu_metadata);
                        Err(e)
                    }
                }
            }
            Err(e) => {
                error!("RPU extraction failed: {}", e);
                // Clean up any partial files
                if rpu_path.exists() {
                    let _ = fs::remove_file(&rpu_path).await;
                }
                Err(e)
            }
        }
    }

    /// Inject RPU metadata into encoded file
    ///
    /// This performs a three-step workflow:
    /// 1. Extract raw HEVC bitstream from MKV container
    /// 2. Inject RPU metadata into raw HEVC using dovi_tool
    /// 3. Remux HEVC+RPU back into MKV with all streams
    pub async fn inject_rpu<P: AsRef<Path>>(
        &self,
        encoded_mkv_path: P,
        rpu_metadata: &RpuMetadata,
        final_output_path: P,
    ) -> Result<()> {
        let dovi_tool = self.dovi_tool.as_ref().ok_or_else(|| {
            Error::DolbyVision(
                "dovi_tool not configured but required for RPU injection".to_string(),
            )
        })?;

        if !rpu_metadata.extracted_successfully {
            return Err(Error::DolbyVision(
                "Cannot inject RPU: metadata extraction was not successful".to_string(),
            ));
        }

        if !rpu_metadata.temp_file.exists() {
            return Err(Error::DolbyVision(format!(
                "RPU file not found: {}",
                rpu_metadata.temp_file.display()
            )));
        }

        let encoded_mkv = encoded_mkv_path.as_ref();
        let final_output = final_output_path.as_ref();

        info!(
            "Injecting RPU metadata into: {}",
            encoded_mkv.display()
        );

        // Step 1: Extract raw HEVC bitstream from MKV
        let temp_hevc = if let Some(parent) = encoded_mkv.parent() {
            parent.join(format!(
                "temp_hevc_{}.hevc",
                Uuid::new_v4()
            ))
        } else {
            PathBuf::from(format!("temp_hevc_{}.hevc", Uuid::new_v4()))
        };

        info!("  Step 1/3: Extracting raw HEVC bitstream from MKV...");
        debug!("    Temp HEVC: {}", temp_hevc.display());

        let extract_status = tokio::process::Command::new("ffmpeg")
            .args([
                "-i", &encoded_mkv.to_string_lossy(),
                "-c:v", "copy",
                "-bsf:v", "hevc_mp4toannexb",
                "-f", "hevc",
                "-y",
                &temp_hevc.to_string_lossy(),
            ])
            .output()
            .await?;

        if !extract_status.status.success() {
            let stderr = String::from_utf8_lossy(&extract_status.stderr);
            let _ = fs::remove_file(&temp_hevc).await;
            return Err(Error::Ffmpeg {
                message: format!("Failed to extract HEVC from MKV: {}", stderr),
            });
        }

        // Step 2: Inject RPU into raw HEVC bitstream
        let hevc_with_rpu = if let Some(parent) = encoded_mkv.parent() {
            parent.join(format!(
                "temp_hevc_rpu_{}.hevc",
                Uuid::new_v4()
            ))
        } else {
            PathBuf::from(format!("temp_hevc_rpu_{}.hevc", Uuid::new_v4()))
        };

        info!("  Step 2/3: Injecting RPU metadata into HEVC bitstream...");
        debug!("    Input HEVC: {}", temp_hevc.display());
        debug!("    RPU file: {}", rpu_metadata.temp_file.display());
        debug!("    Output HEVC+RPU: {}", hevc_with_rpu.display());

        match dovi_tool
            .inject_rpu(&temp_hevc, &rpu_metadata.temp_file, &hevc_with_rpu)
            .await
        {
            Ok(_) => {
                info!("    RPU injection successful!");
            }
            Err(e) => {
                let _ = fs::remove_file(&temp_hevc).await;
                let _ = fs::remove_file(&hevc_with_rpu).await;
                return Err(e);
            }
        }

        // Clean up intermediate HEVC file
        let _ = fs::remove_file(&temp_hevc).await;

        // Step 3: Remux HEVC+RPU back into MKV with all streams
        info!("  Step 3/3: Remuxing HEVC+RPU back into MKV with all streams...");
        debug!("    Source MKV (for streams): {}", encoded_mkv.display());
        debug!("    HEVC+RPU: {}", hevc_with_rpu.display());
        debug!("    Final output: {}", final_output.display());

        let remux_status = tokio::process::Command::new("ffmpeg")
            .args([
                "-f", "hevc",           // Explicitly specify raw HEVC input format
                "-fflags", "+genpts",   // Generate presentation timestamps for raw HEVC
                "-i", &hevc_with_rpu.to_string_lossy(),
                "-i", &encoded_mkv.to_string_lossy(),
                "-map", "0:v:0",        // Video from HEVC+RPU
                "-map", "1:a?",         // All audio streams from original MKV
                "-map", "1:s?",         // All subtitle streams from original MKV
                "-map", "1:t?",         // All attachments from original MKV
                "-map", "1:d?",         // All data streams from original MKV
                "-c", "copy",           // Copy all streams without re-encoding
                "-map_metadata", "1",   // Copy metadata from original MKV
                "-map_chapters", "1",   // Copy chapters from original MKV
                "-y",
                &final_output.to_string_lossy(),
            ])
            .output()
            .await?;

        // Clean up HEVC+RPU file
        let _ = fs::remove_file(&hevc_with_rpu).await;

        if !remux_status.status.success() {
            let stderr = String::from_utf8_lossy(&remux_status.stderr);
            let stdout = String::from_utf8_lossy(&remux_status.stdout);

            error!("FFmpeg remux failed!");
            if !stderr.is_empty() {
                error!("FFmpeg stderr: {}", stderr);
            }
            if !stdout.is_empty() {
                debug!("FFmpeg stdout: {}", stdout);
            }

            return Err(Error::Ffmpeg {
                message: format!("Failed to remux HEVC+RPU into MKV: {}", stderr),
            });
        }

        info!("Successfully injected Dolby Vision RPU metadata!");
        info!("  Profile: {}", rpu_metadata.profile.as_str());
        info!("  Final file: {}", final_output.display());

        Ok(())
    }

    /// Clean up temporary RPU file
    pub fn cleanup_rpu(&self, rpu_metadata: &RpuMetadata) {
        if rpu_metadata.temp_file.exists() {
            match std::fs::remove_file(&rpu_metadata.temp_file) {
                Ok(_) => debug!("Cleaned up RPU file: {}", rpu_metadata.temp_file.display()),
                Err(e) => warn!(
                    "Failed to clean up RPU file {}: {}",
                    rpu_metadata.temp_file.display(),
                    e
                ),
            }
        }
    }

    /// Clean up all RPU files in temp directory (emergency cleanup)
    pub async fn cleanup_all_rpu_files(&self) -> Result<()> {
        if !self.temp_dir.exists() {
            return Ok(());
        }

        let mut dir = fs::read_dir(&self.temp_dir).await?;
        let mut cleaned_count = 0;

        while let Some(entry) = dir.next_entry().await? {
            let path = entry.path();
            if let Some(filename) = path.file_name() {
                if filename.to_string_lossy().starts_with("rpu_")
                    && filename.to_string_lossy().ends_with(".bin")
                {
                    match fs::remove_file(&path).await {
                        Ok(_) => {
                            cleaned_count += 1;
                            debug!("Cleaned up orphaned RPU file: {}", path.display());
                        }
                        Err(e) => warn!("Failed to clean up RPU file {}: {}", path.display(), e),
                    }
                }
            }
        }

        if cleaned_count > 0 {
            info!("Cleaned up {} orphaned RPU files", cleaned_count);
        }

        Ok(())
    }

    /// Estimate RPU processing overhead
    pub fn estimate_processing_overhead(&self, dv_info: &DolbyVisionInfo) -> f32 {
        if !dv_info.needs_rpu_processing() {
            return 0.0;
        }

        match dv_info.profile {
            DolbyVisionProfile::Profile7 => 1.8, // Higher overhead for dual-layer
            DolbyVisionProfile::Profile81 | DolbyVisionProfile::Profile82 => 1.3, // Moderate overhead
            DolbyVisionProfile::Profile5 => 1.2,                                  // Lower overhead
            _ => 1.0,
        }
    }

    /// Check if we have the required tools for RPU processing
    pub async fn check_rpu_capability(&self) -> Result<bool> {
        match &self.dovi_tool {
            Some(tool) => tool.check_availability().await.map(|_| true),
            None => Ok(false),
        }
    }
}

impl Drop for RpuManager {
    fn drop(&mut self) {
        // Best effort cleanup on drop
        if self.temp_dir.exists() {
            let _ = std::thread::spawn({
                let temp_dir = self.temp_dir.clone();
                move || {
                    let rt = tokio::runtime::Handle::try_current();
                    if let Ok(handle) = rt {
                        handle.block_on(async {
                            let mut dir = match tokio::fs::read_dir(&temp_dir).await {
                                Ok(dir) => dir,
                                Err(_) => return,
                            };

                            while let Ok(Some(entry)) = dir.next_entry().await {
                                let path = entry.path();
                                if let Some(filename) = path.file_name() {
                                    if filename.to_string_lossy().starts_with("rpu_") {
                                        let _ = tokio::fs::remove_file(&path).await;
                                    }
                                }
                            }
                        });
                    }
                }
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_rpu_metadata_creation() {
        let temp_path = PathBuf::from("/tmp/test.rpu");
        let metadata = RpuMetadata::new(temp_path.clone(), DolbyVisionProfile::Profile81);

        assert_eq!(metadata.temp_file, temp_path);
        assert_eq!(metadata.profile, DolbyVisionProfile::Profile81);
        assert!(!metadata.extracted_successfully);
        assert_eq!(metadata.frame_count, None);
    }

    #[tokio::test]
    async fn test_rpu_manager_temp_dir_creation() {
        let temp_dir = tempdir().unwrap();
        let rpu_temp_dir = temp_dir.path().join("rpu_test");
        let manager = RpuManager::new(rpu_temp_dir.clone(), None);

        assert!(!rpu_temp_dir.exists());
        manager.ensure_temp_dir().await.unwrap();
        assert!(rpu_temp_dir.exists());
    }

    #[test]
    fn test_processing_overhead_estimation() {
        let manager = RpuManager::new(PathBuf::new(), None);

        let dv_info_p7 = DolbyVisionInfo {
            profile: DolbyVisionProfile::Profile7,
            has_rpu: true,
            rpu_present: true,
            has_enhancement_layer: true,
            el_present: true,
            ..Default::default()
        };

        let overhead = manager.estimate_processing_overhead(&dv_info_p7);
        assert_eq!(overhead, 1.8);
    }
}
