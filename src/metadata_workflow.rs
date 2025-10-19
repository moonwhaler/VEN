/// Metadata Workflow Manager
///
/// This module handles the complete workflow for external metadata tools:
/// 1. Check tool availability and log status
/// 2. Extract metadata before encoding (dovi_tool extract-rpu, hdr10plus_tool extract)
/// 3. Provide metadata paths for x265 encoding (--dhdr10-info, etc.)
/// 4. Inject metadata after encoding (dovi_tool inject-rpu, hdr10plus_tool inject)
/// 5. Clean up temporary files
use crate::analysis::dolby_vision::DolbyVisionInfo;
use crate::config::Config;
use crate::dolby_vision::{
    rpu::RpuManager,
    tools::{DoviTool, DoviToolConfig},
    RpuMetadata,
};
use crate::hdr::types::HdrAnalysisResult;
use crate::hdr10plus::{manager::Hdr10PlusManager, Hdr10PlusProcessingResult};
use crate::mkvmerge::MkvMergeTool;
use crate::utils::Result;
use crate::ContentEncodingApproach;
use std::path::{Path, PathBuf};
use tracing::{debug, info, warn};

#[derive(Debug, Clone)]
pub struct ExtractedMetadata {
    pub dolby_vision: Option<RpuMetadata>,
    pub hdr10_plus: Option<Hdr10PlusProcessingResult>,
    pub temp_dir: PathBuf,
}

impl ExtractedMetadata {
    pub fn none(temp_dir: PathBuf) -> Self {
        Self {
            dolby_vision: None,
            hdr10_plus: None,
            temp_dir,
        }
    }

    pub fn has_metadata(&self) -> bool {
        self.dolby_vision.is_some() || self.hdr10_plus.is_some()
    }

    pub fn cleanup(&self) {
        if let Some(ref dv) = self.dolby_vision {
            if dv.temp_file.exists() {
                let _ = std::fs::remove_file(&dv.temp_file);
                debug!("Cleaned up DV RPU file: {}", dv.temp_file.display());
            }
        }

        if let Some(ref hdr10plus) = self.hdr10_plus {
            if hdr10plus.metadata_file.exists() {
                let _ = std::fs::remove_file(&hdr10plus.metadata_file);
                debug!(
                    "Cleaned up HDR10+ metadata file: {}",
                    hdr10plus.metadata_file.display()
                );
            }
        }
    }
}

pub struct MetadataWorkflowManager {
    rpu_manager: Option<RpuManager>,
    hdr10plus_manager: Option<Hdr10PlusManager>,
    temp_dir: PathBuf,
    tools_available: ToolAvailability,
}

#[derive(Debug, Clone)]
pub struct ToolAvailability {
    pub dovi_tool: bool,
    pub hdr10plus_tool: bool,
}

impl MetadataWorkflowManager {
    pub async fn new(config: &Config) -> Result<Self> {
        let temp_dir = PathBuf::from(&config.app.temp_dir);

        // Initialize RPU manager if Dolby Vision is enabled
        let rpu_manager = if config
            .analysis
            .dolby_vision
            .as_ref()
            .is_some_and(|dv| dv.enabled)
        {
            let dovi_tool = if let Some(ref dv_config) = config.tools.dovi_tool {
                // Convert from config::types::DoviToolConfig to dolby_vision::tools::DoviToolConfig
                let tool_config = DoviToolConfig {
                    path: dv_config.path.clone(),
                    timeout_seconds: dv_config.timeout_seconds,
                    extract_args: dv_config.extract_args.clone(),
                    inject_args: dv_config.inject_args.clone(),
                };
                Some(DoviTool::new(tool_config))
            } else {
                None
            };

            let mkvmerge_tool = config.tools.mkvmerge.as_ref().map(|mkv_config| MkvMergeTool::new(mkv_config.clone()));

            Some(RpuManager::new(temp_dir.clone(), dovi_tool, mkvmerge_tool))
        } else {
            None
        };

        // Initialize HDR10+ manager if HDR10+ is enabled
        let hdr10plus_manager = if config
            .analysis
            .hdr10_plus
            .as_ref()
            .is_some_and(|h| h.enabled)
        {
            Some(Hdr10PlusManager::new(
                temp_dir.clone(),
                config.tools.hdr10plus_tool.clone(),
            ))
        } else {
            None
        };

        let mut workflow_manager = Self {
            rpu_manager,
            hdr10plus_manager,
            temp_dir,
            tools_available: ToolAvailability {
                dovi_tool: false,
                hdr10plus_tool: false,
            },
        };

        // Check tool availability and log status
        workflow_manager.check_and_log_tool_availability().await?;

        Ok(workflow_manager)
    }

    async fn check_and_log_tool_availability(&mut self) -> Result<()> {
        info!("Checking external metadata tool availability...");

        // Check dovi_tool availability
        if let Some(ref manager) = self.rpu_manager {
            match manager.check_rpu_capability().await {
                Ok(available) => {
                    self.tools_available.dovi_tool = available;
                    if available {
                        info!(
                            "dovi_tool: AVAILABLE - Dolby Vision RPU extraction/injection enabled"
                        );
                    } else {
                        info!("dovi_tool: NOT AVAILABLE - Dolby Vision will use x265 built-in parameters only");
                    }
                }
                Err(e) => {
                    warn!("dovi_tool: ERROR checking availability: {}", e);
                    self.tools_available.dovi_tool = false;
                    info!(
                        "dovi_tool: DISABLED - Dolby Vision will use x265 built-in parameters only"
                    );
                }
            }
        } else {
            info!("dovi_tool: DISABLED in configuration");
        }

        // Check hdr10plus_tool availability
        if let Some(ref manager) = self.hdr10plus_manager {
            match manager.check_hdr10plus_capability().await {
                Ok(available) => {
                    self.tools_available.hdr10plus_tool = available;
                    if available {
                        info!("hdr10plus_tool: AVAILABLE - HDR10+ dynamic metadata extraction/injection enabled");
                    } else {
                        info!("hdr10plus_tool: NOT AVAILABLE - HDR10+ will use x265 built-in parameters only");
                    }
                }
                Err(e) => {
                    warn!("hdr10plus_tool: ERROR checking availability: {}", e);
                    self.tools_available.hdr10plus_tool = false;
                    info!(
                        "hdr10plus_tool: DISABLED - HDR10+ will use x265 built-in parameters only"
                    );
                }
            }
        } else {
            info!("hdr10plus_tool: DISABLED in configuration");
        }

        // Summary
        if self.tools_available.dovi_tool || self.tools_available.hdr10plus_tool {
            info!("External metadata tools are ready - enhanced preservation enabled!");
        } else {
            info!("No external tools available - using x265 built-in HDR support only");
        }

        Ok(())
    }

    /// Extract metadata from input file before encoding
    pub async fn extract_metadata<P: AsRef<Path>>(
        &self,
        input_path: P,
        approach: &ContentEncodingApproach,
        dv_info: &DolbyVisionInfo,
        hdr_analysis: &HdrAnalysisResult,
    ) -> Result<ExtractedMetadata> {
        info!("Starting pre-encoding metadata extraction phase");

        let mut extracted = ExtractedMetadata::none(self.temp_dir.clone());

        match approach {
            ContentEncodingApproach::DolbyVision(_) => {
                info!("Processing Dolby Vision content");
                extracted.dolby_vision = self
                    .extract_dolby_vision_metadata(&input_path, dv_info)
                    .await?;
            }
            ContentEncodingApproach::DolbyVisionWithHDR10Plus(_, _) => {
                info!("Processing dual format content (Dolby Vision + HDR10+)");
                // Extract both DV and HDR10+ metadata for dual format
                extracted.dolby_vision = self
                    .extract_dolby_vision_metadata(&input_path, dv_info)
                    .await?;
                extracted.hdr10_plus = self
                    .extract_hdr10plus_metadata(&input_path, hdr_analysis)
                    .await?;
            }
            ContentEncodingApproach::HDR(hdr_result) => {
                // Check if this is HDR10+ content
                if hdr_result.metadata.format == crate::hdr::types::HdrFormat::HDR10Plus {
                    info!("Processing HDR10+ content");
                    extracted.hdr10_plus = self
                        .extract_hdr10plus_metadata(&input_path, hdr_analysis)
                        .await?;
                } else {
                    info!("Processing standard HDR10 content (no external tools needed)");
                }
            }
            ContentEncodingApproach::SDR => {
                info!("Processing SDR content (no metadata extraction needed)");
            }
        }

        if extracted.has_metadata() {
            info!("Metadata extraction phase completed - external metadata ready for encoding");
        } else {
            info!(
                "No external metadata extracted - encoding will use x265 built-in parameters only"
            );
        }

        Ok(extracted)
    }

    async fn extract_dolby_vision_metadata<P: AsRef<Path>>(
        &self,
        input_path: P,
        dv_info: &DolbyVisionInfo,
    ) -> Result<Option<RpuMetadata>> {
        if !self.tools_available.dovi_tool {
            info!("Skipping Dolby Vision RPU extraction - dovi_tool not available");
            info!("   Encoding will continue with HDR10 fallback parameters");
            return Ok(None);
        }

        if !dv_info.is_dolby_vision() || !dv_info.rpu_present {
            debug!("No Dolby Vision RPU to extract from this content");
            return Ok(None);
        }

        let Some(ref manager) = self.rpu_manager else {
            warn!("RPU manager not initialized despite tool being available");
            return Ok(None);
        };

        info!("Extracting Dolby Vision RPU metadata using dovi_tool...");
        info!("   Profile: {}", dv_info.profile.as_str());

        match manager.extract_rpu(&input_path, dv_info).await {
            Ok(metadata) => {
                if let Some(ref meta) = metadata {
                    info!("Dolby Vision RPU extraction successful!");
                    info!(
                        "   Profile: {}, File: {}, Size: {} bytes",
                        meta.profile.as_str(),
                        meta.temp_file.display(),
                        meta.file_size.unwrap_or(0)
                    );
                }
                Ok(metadata)
            }
            Err(e) => {
                warn!("Dolby Vision RPU extraction failed:");

                // Log the detailed error message with tool-specific information
                let error_details = e.to_string();
                for line in error_details.lines() {
                    warn!("   {}", line);
                }

                warn!("   This is not necessarily a critical error - some content may not have extractable RPU");
                warn!("   Encoding will continue with HDR10 fallback parameters");
                Ok(None)
            }
        }
    }

    async fn extract_hdr10plus_metadata<P: AsRef<Path>>(
        &self,
        input_path: P,
        hdr_analysis: &HdrAnalysisResult,
    ) -> Result<Option<Hdr10PlusProcessingResult>> {
        if !self.tools_available.hdr10plus_tool {
            info!("Skipping HDR10+ metadata extraction - hdr10plus_tool not available");
            info!("   Encoding will continue with HDR10 fallback parameters");
            return Ok(None);
        }

        let Some(ref manager) = self.hdr10plus_manager else {
            warn!("HDR10+ manager not initialized despite tool being available");
            return Ok(None);
        };

        info!("Extracting HDR10+ dynamic metadata using hdr10plus_tool...");

        match manager
            .extract_hdr10plus_metadata(&input_path, hdr_analysis)
            .await
        {
            Ok(metadata) => {
                if let Some(ref meta) = metadata {
                    info!("HDR10+ metadata extraction successful!");
                    info!(
                        "   Frames: {}, Curves: {}, File: {}",
                        meta.metadata.get_frame_count(),
                        meta.curve_count,
                        meta.metadata_file.display()
                    );
                }
                Ok(metadata)
            }
            Err(e) => {
                warn!("HDR10+ metadata extraction failed:");

                // Log the detailed error message with tool-specific information
                let error_details = e.to_string();
                for line in error_details.lines() {
                    warn!("   {}", line);
                }

                warn!("   This is not necessarily a critical error - some content may not have extractable metadata");
                warn!("   Encoding will continue with HDR10 fallback parameters");
                Ok(None)
            }
        }
    }

    /// Build x265 parameters including external metadata file paths
    /// Based on dovi_tool and hdr10plus_tool documentation
    pub fn build_external_metadata_params(
        &self,
        extracted: &ExtractedMetadata,
    ) -> Vec<(String, String)> {
        let mut params = Vec::new();

        // Add Dolby Vision RPU parameter if available
        // This will be passed to x265 during encoding (though x265 built-in DV support is limited)
        if let Some(ref dv_meta) = extracted.dolby_vision {
            if dv_meta.extracted_successfully && dv_meta.temp_file.exists() {
                // Note: x265 has limited built-in DV support, but we store the path for potential future use
                // The main workflow is: extract -> encode without DV -> inject RPU post-encoding
                debug!(
                    "Dolby Vision RPU available for post-encoding injection: {}",
                    dv_meta.temp_file.display()
                );
                debug!("   Profile: {}", dv_meta.profile.as_str());
            }
        }

        // Add HDR10+ metadata parameter if available
        // x265 supports --dhdr10-info parameter for HDR10+ metadata files
        if let Some(ref hdr10plus_meta) = extracted.hdr10_plus {
            if hdr10plus_meta.extraction_successful && hdr10plus_meta.metadata_file.exists() {
                params.push((
                    "dhdr10-info".to_string(),
                    hdr10plus_meta.metadata_file.to_string_lossy().to_string(),
                ));
                info!(
                    "Added HDR10+ metadata parameter for x265: --dhdr10-info {}",
                    hdr10plus_meta.metadata_file.display()
                );
                debug!(
                    "   Frames with metadata: {}",
                    hdr10plus_meta.metadata.get_frame_count()
                );
            }
        }

        if !params.is_empty() {
            info!("External metadata parameters ready for x265 encoding");
        } else {
            debug!("No external metadata parameters to add - using x265 built-in HDR support only");
        }

        params
    }

    /// Post-encoding step: inject metadata back into the final file
    /// This is the key step that was missing in the original implementation
    ///
    /// # Parameters
    /// * `fps` - Framerate of the video, required for proper RPU injection timing
    pub async fn inject_metadata<P: AsRef<Path>>(
        &self,
        encoded_path: P,
        final_output_path: P,
        extracted: &ExtractedMetadata,
        fps: f32,
    ) -> Result<()> {
        // If no metadata was extracted, just rename/move the file
        if !extracted.has_metadata() {
            if encoded_path.as_ref() != final_output_path.as_ref() {
                tokio::fs::rename(&encoded_path, &final_output_path).await?;
                debug!("Moved encoded file to final location (no metadata injection needed)");
            }
            return Ok(());
        }

        info!("Starting post-encoding metadata injection phase");

        // Handle Dolby Vision RPU injection first (most critical)
        if let Some(ref dv_meta) = extracted.dolby_vision {
            if dv_meta.extracted_successfully && self.tools_available.dovi_tool {
                info!("Injecting Dolby Vision RPU metadata using dovi_tool...");
                info!("   Video framerate: {} fps (required for timing synchronization)", fps);
                if let Some(ref manager) = self.rpu_manager {
                    match manager
                        .inject_rpu(&encoded_path, dv_meta, &final_output_path, fps)
                        .await
                    {
                        Ok(_) => {
                            info!("Dolby Vision RPU injection successful!");
                            info!("   Final file: {}", final_output_path.as_ref().display());
                            info!("   Profile: {}", dv_meta.profile.as_str());

                            // If we also had HDR10+ metadata, log that it was included during encoding
                            if let Some(ref hdr10plus_meta) = extracted.hdr10_plus {
                                if hdr10plus_meta.extraction_successful {
                                    info!("HDR10+ metadata was included during x265 encoding (--dhdr10-info)");
                                    info!("   This is a dual-format Dolby Vision + HDR10+ file!");
                                }
                            }
                            return Ok(());
                        }
                        Err(e) => {
                            warn!("Dolby Vision RPU injection failed: {}", e);
                            warn!("   Falling back to encoded file without RPU injection");
                        }
                    }
                }
            }
        }

        // If we reach here, either DV injection failed or there was only HDR10+ metadata
        if let Some(ref hdr10plus_meta) = extracted.hdr10_plus {
            if hdr10plus_meta.extraction_successful {
                info!("HDR10+ metadata was successfully included during x265 encoding");
                info!("   No post-encoding injection needed for HDR10+ (handled by --dhdr10-info)");
            }
        }

        // Fallback: move the encoded file to final location
        if encoded_path.as_ref() != final_output_path.as_ref() {
            tokio::fs::rename(&encoded_path, &final_output_path).await?;
            info!(
                "Moved encoded file to final location: {}",
                final_output_path.as_ref().display()
            );
        }

        Ok(())
    }

    /// Get tool availability status for logging
    pub fn get_tool_availability(&self) -> &ToolAvailability {
        &self.tools_available
    }

    /// Check if we should use a temporary output path for post-processing
    pub fn needs_post_processing(&self, extracted: &ExtractedMetadata) -> bool {
        // We need post-processing if we have Dolby Vision RPU to inject
        extracted
            .dolby_vision
            .as_ref()
            .is_some_and(|dv| dv.extracted_successfully && self.tools_available.dovi_tool)
    }

    /// Generate a temporary output path for post-processing alongside the source file
    pub fn get_temp_output_path<P: AsRef<Path>>(&self, final_path: P) -> PathBuf {
        let final_path = final_path.as_ref();

        if let Some(filename) = final_path.file_name() {
            let temp_filename = format!("temp_encode_{}", filename.to_string_lossy());
            if let Some(parent) = final_path.parent() {
                parent.join(temp_filename)
            } else {
                PathBuf::from(temp_filename)
            }
        } else {
            final_path
                .parent()
                .unwrap_or(Path::new("."))
                .join("temp_encode_output.mkv")
        }
    }

    /// Clean up all temporary files
    pub async fn cleanup(&self) -> Result<()> {
        debug!("Cleaning up temporary metadata files...");

        if let Some(ref manager) = self.rpu_manager {
            if let Err(e) = manager.cleanup_all_rpu_files().await {
                warn!("Failed to cleanup RPU files: {}", e);
            }
        }

        if let Some(ref manager) = self.hdr10plus_manager {
            if let Err(e) = manager.cleanup().await {
                warn!("Failed to cleanup HDR10+ files: {}", e);
            }
        }

        Ok(())
    }
}
