use super::metadata::{Hdr10PlusMetadata, Hdr10PlusProcessingResult};
use super::tools::{Hdr10PlusTool, Hdr10PlusToolConfig};
use crate::analysis::dolby_vision::DolbyVisionInfo;
use crate::hdr::types::{HdrAnalysisResult, HdrFormat};
use crate::utils::{Error, Result};
use std::path::{Path, PathBuf};
use tracing::{debug, info, warn};
use uuid::Uuid;

/// High-level manager for HDR10+ dynamic metadata processing
pub struct Hdr10PlusManager {
    tool: Option<Hdr10PlusTool>,
    temp_dir: PathBuf,
    _tool_config: Hdr10PlusToolConfig,
}

impl Hdr10PlusManager {
    /// Create a new HDR10+ manager
    pub fn new(temp_dir: PathBuf, tool_config: Option<Hdr10PlusToolConfig>) -> Self {
        let tool_cfg = tool_config.unwrap_or_default();
        let tool = Some(Hdr10PlusTool::new(tool_cfg.clone()));

        Self {
            tool,
            temp_dir,
            _tool_config: tool_cfg,
        }
    }

    /// Check if HDR10+ processing capability is available
    pub async fn check_hdr10plus_capability(&self) -> Result<bool> {
        if let Some(ref tool) = self.tool {
            tool.check_availability().await
        } else {
            Ok(false)
        }
    }

    /// Extract HDR10+ dynamic metadata from video file
    pub async fn extract_hdr10plus_metadata<P: AsRef<Path>>(
        &self,
        input_video: P,
        hdr_result: &HdrAnalysisResult,
    ) -> Result<Option<Hdr10PlusProcessingResult>> {
        // Process HDR10+ content, and also attempt extraction for HDR10 content
        // that may contain HDR10+ dynamic metadata
        if !matches!(
            hdr_result.metadata.format,
            HdrFormat::HDR10Plus | HdrFormat::HDR10
        ) {
            debug!(
                "Skipping HDR10+ extraction - content format is {:?}",
                hdr_result.metadata.format
            );
            return Ok(None);
        }

        let Some(ref tool) = self.tool else {
            warn!("HDR10+ tool not available for metadata extraction");
            return Ok(None);
        };

        // Check tool availability
        if !tool.check_availability().await? {
            warn!("hdr10plus_tool is not available - skipping HDR10+ metadata extraction");
            return Ok(None);
        }

        let input_path = input_video.as_ref();
        info!(
            "Extracting HDR10+ dynamic metadata from: {}",
            input_path.display()
        );

        // Generate HDR10+ metadata file alongside the source video
        let input_stem = input_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("video");
        let metadata_id = Uuid::new_v4().to_string();
        let metadata_filename = format!("{}_hdr10plus_metadata_{}.json", input_stem, metadata_id);

        let metadata_file = if let Some(parent) = input_path.parent() {
            parent.join(metadata_filename)
        } else {
            PathBuf::from(metadata_filename)
        };

        match tool.extract_metadata(input_path, &metadata_file).await {
            Ok(_) => {
                // Load and parse the extracted metadata
                match Hdr10PlusMetadata::from_json_file(&metadata_file).await {
                    Ok(metadata) => {
                        // Validate the metadata
                        if let Err(e) = metadata.validate() {
                            warn!("HDR10+ metadata validation failed: {}", e);
                            return Ok(None);
                        }

                        let file_size = tokio::fs::metadata(&metadata_file)
                            .await
                            .map(|m| m.len())
                            .ok();

                        let result = Hdr10PlusProcessingResult {
                            metadata_file,
                            metadata,
                            extraction_successful: true,
                            file_size,
                            curve_count: 0, // Will be calculated in constructor
                            scene_count: 0, // Will be calculated in constructor
                        };

                        info!(
                            "Successfully extracted HDR10+ metadata: {} frames, {} scenes",
                            result.metadata.get_frame_count(),
                            result.metadata.get_scene_count()
                        );

                        Ok(Some(result))
                    }
                    Err(e) => {
                        warn!("Failed to parse extracted HDR10+ metadata: {}", e);
                        // Clean up the file
                        let _ = tokio::fs::remove_file(&metadata_file).await;
                        Ok(None)
                    }
                }
            }
            Err(e) => {
                // Check if this is the expected "no dynamic metadata" case
                let error_message = e.to_string();
                if error_message.contains("File doesn't contain dynamic metadata")
                    || error_message.contains("No dynamic metadata found")
                    || error_message.contains("Tool failed with exit code exit status: 1")
                {
                    debug!("No HDR10+ dynamic metadata found in file - this is normal for HDR10 content");
                    info!("No HDR10+ dynamic metadata detected (standard HDR10 content)");
                } else {
                    warn!("HDR10+ metadata extraction failed: {}", e);
                }
                Ok(None)
            }
        }
    }

    /// Process dual Dolby Vision + HDR10+ content
    pub async fn process_dual_format<P: AsRef<Path>>(
        &self,
        input_video: P,
        dv_info: &DolbyVisionInfo,
        hdr_result: &HdrAnalysisResult,
    ) -> Result<Option<Hdr10PlusProcessingResult>> {
        if !dv_info.is_dolby_vision()
            || !matches!(
                hdr_result.metadata.format,
                HdrFormat::HDR10Plus | HdrFormat::HDR10
            )
        {
            debug!("Not dual DV+HDR10/HDR10+ content - skipping dual processing");
            return self
                .extract_hdr10plus_metadata(input_video, hdr_result)
                .await;
        }

        info!("Processing dual Dolby Vision + HDR10+ content");

        // For dual format, we need to be extra careful about metadata preservation
        match self
            .extract_hdr10plus_metadata(&input_video, hdr_result)
            .await
        {
            Ok(Some(result)) => {
                // Metadata successfully extracted for dual format
                info!(
                    "Dual format processing complete: DV Profile {} + HDR10+ ({} frames)",
                    dv_info.profile.as_str(),
                    result.metadata.get_frame_count()
                );

                Ok(Some(result))
            }
            Ok(None) => {
                warn!("Failed to extract HDR10+ metadata from dual format content");
                Ok(None)
            }
            Err(e) => Err(e),
        }
    }

    /// Generate x265 parameters for HDR10+ encoding
    pub fn build_hdr10plus_x265_params(
        &self,
        hdr10plus_result: &Hdr10PlusProcessingResult,
    ) -> Result<Vec<(String, String)>> {
        if !hdr10plus_result.extraction_successful {
            return Err(Error::encoding(
                "Cannot build x265 params - HDR10+ extraction failed".to_string(),
            ));
        }

        let params = vec![
            // Core HDR10+ parameter - path to the JSON metadata file
            (
                "dhdr10-info".to_string(),
                hdr10plus_result.metadata_file.to_string_lossy().to_string(),
            ),
            // HDR10+ specific optimizations
            ("hdr10plus-opt".to_string(), "1".to_string()),
            // Enhanced rate control for dynamic metadata
            ("rc-lookahead".to_string(), "60".to_string()), // Longer lookahead
            ("bframes".to_string(), "8".to_string()),       // More B-frames for better compression
            ("b-adapt".to_string(), "2".to_string()),
            // Quality optimizations for HDR10+ content
            ("psy-rd".to_string(), "2.5".to_string()), // Higher psychovisual optimization
            ("psy-rdoq".to_string(), "1.0".to_string()),
            ("aq-mode".to_string(), "3".to_string()), // Adaptive quantization mode 3
            ("aq-strength".to_string(), "1.0".to_string()),
            // Enhanced motion estimation for dynamic content
            ("me".to_string(), "umh".to_string()),
            ("subme".to_string(), "5".to_string()),
            ("merange".to_string(), "64".to_string()),
            // Transform optimizations
            ("rect".to_string(), "".to_string()),
            ("amp".to_string(), "".to_string()),
            // Additional quality enhancements
            ("strong-intra-smoothing".to_string(), "".to_string()),
            ("weightb".to_string(), "".to_string()),
            ("weightp".to_string(), "2".to_string()),
        ];

        info!("Generated {} HDR10+ x265 parameters", params.len());
        Ok(params)
    }

    /// Build x265 parameters for dual Dolby Vision + HDR10+ encoding
    pub fn build_dual_format_x265_params(
        &self,
        dv_info: &DolbyVisionInfo,
        hdr10plus_result: &Hdr10PlusProcessingResult,
        dovi_rpu_path: Option<&Path>,
    ) -> Result<Vec<(String, String)>> {
        let mut params = Vec::new();

        // Dolby Vision parameters first
        if let Some(rpu_path) = dovi_rpu_path {
            params.push((
                "dolby-vision-rpu".to_string(),
                rpu_path.to_string_lossy().to_string(),
            ));
            params.push((
                "dolby-vision-profile".to_string(),
                dv_info.profile.as_str().to_string(),
            ));
        }

        // HDR10+ parameters
        let hdr10plus_params = self.build_hdr10plus_x265_params(hdr10plus_result)?;
        params.extend(hdr10plus_params);

        // VBV constraints now come from profile settings only
        // No hardcoded VBV constraints - profiles control VBV settings

        // Ultra-conservative quality settings for dual metadata preservation
        params.push(("crf".to_string(), "16".to_string())); // Very low CRF
        params.push(("preset".to_string(), "veryslow".to_string())); // Highest quality preset

        // Maximum rate-distortion optimization
        params.push(("rd".to_string(), "6".to_string())); // Highest RD level
        params.push(("rdoq-level".to_string(), "2".to_string()));

        // Enhanced B-frame settings for dual format
        params.push(("bframes".to_string(), "10".to_string())); // Maximum B-frames
        params.push(("b-pyramid".to_string(), "".to_string()));
        params.push(("b-adapt".to_string(), "2".to_string()));

        info!(
            "Generated {} dual format (DV+HDR10+) x265 parameters",
            params.len()
        );
        Ok(params)
    }

    /// Clean up temporary HDR10+ files
    pub async fn cleanup(&self) -> Result<()> {
        debug!(
            "Cleaning up HDR10+ temporary files in: {}",
            self.temp_dir.display()
        );

        let mut dir = tokio::fs::read_dir(&self.temp_dir)
            .await
            .map_err(Error::Io)?;

        while let Some(entry) = dir.next_entry().await.map_err(Error::Io)? {
            let path = entry.path();
            if let Some(filename) = path.file_name().and_then(|n| n.to_str()) {
                if filename.starts_with("hdr10plus_metadata_") && filename.ends_with(".json") {
                    debug!("Removing HDR10+ metadata file: {}", path.display());
                    if let Err(e) = tokio::fs::remove_file(&path).await {
                        warn!(
                            "Failed to remove HDR10+ metadata file {}: {}",
                            path.display(),
                            e
                        );
                    }
                }
            }
        }

        debug!("HDR10+ cleanup completed");
        Ok(())
    }

    /// Estimate processing overhead for HDR10+ content
    pub fn estimate_processing_overhead(
        &self,
        hdr_result: &HdrAnalysisResult,
        hdr10plus_result: Option<&Hdr10PlusProcessingResult>,
    ) -> f32 {
        match hdr_result.metadata.format {
            HdrFormat::HDR10Plus => {
                if let Some(result) = hdr10plus_result {
                    result.estimate_processing_overhead()
                } else {
                    // Fallback if we couldn't extract HDR10+ metadata
                    1.4
                }
            }
            _ => 1.0, // No HDR10+ processing overhead
        }
    }
}
