use crate::config::types::MkvMergeConfig;
use crate::utils::{Result, ToolRunner};
use std::path::Path;
use tracing::{debug, info};

pub struct MkvMergeTool {
    tool: ToolRunner,
}

impl MkvMergeTool {
    pub fn new(config: MkvMergeConfig) -> Self {
        Self {
            tool: ToolRunner::new(crate::utils::ToolConfig {
                path: config.path,
                timeout_seconds: config.timeout_seconds,
                extract_args: None,
                inject_args: None,
            }),
        }
    }

    pub async fn check_availability(&self) -> Result<bool> {
        match self.tool.check_availability("--version", "mkvmerge").await {
            Ok(_) => Ok(true),
            Err(_) => Ok(false),
        }
    }

    /// Remux raw HEVC+RPU bitstream with streams from original MKV
    ///
    /// This takes a raw HEVC file (with RPU injected) and combines it with
    /// audio, subtitles, chapters, and other streams from the original MKV.
    ///
    /// # Parameters
    /// * `hevc_file` - Raw HEVC+RPU bitstream file
    /// * `source_mkv` - Original MKV file containing audio/subtitle/chapter streams
    /// * `output_mkv` - Final output MKV file path
    /// * `fps` - Video framerate for proper timing
    pub async fn remux_hevc_with_streams<P1: AsRef<Path>, P2: AsRef<Path>, P3: AsRef<Path>>(
        &self,
        hevc_file: P1,
        source_mkv: P2,
        output_mkv: P3,
        fps: f32,
    ) -> Result<()> {
        let hevc_path = hevc_file.as_ref().to_string_lossy();
        let source_path = source_mkv.as_ref().to_string_lossy();
        let output_path = output_mkv.as_ref().to_string_lossy();

        info!(
            "Remuxing HEVC+RPU with mkvmerge: {} + {} -> {}",
            hevc_path, source_path, output_path
        );
        info!("Running mkvmerge (this may take a moment)...");
        debug!("Video framerate: {} fps", fps);

        // Build mkvmerge command:
        // - Output file
        // - HEVC video track with framerate
        // - No audio/subtitles from HEVC (it has none)
        // - All audio tracks from source MKV
        // - All subtitle tracks from source MKV
        // - Chapters from source MKV
        let fps_str = format!("{}fps", fps);
        let args = vec![
            "-o".to_string(),
            output_path.to_string(),
            "--default-duration".to_string(),
            format!("0:{}", fps_str),
            "--no-audio".to_string(),
            "--no-subtitles".to_string(),
            "--no-chapters".to_string(),
            hevc_path.to_string(),
            "-D".to_string(), // No video from source
            source_path.to_string(),
        ];

        debug!("  mkvmerge command: {} {}", self.tool.config().path, args.join(" "));

        self.tool
            .run_with_custom_args(&args, &None, Some(output_mkv))
            .await?;

        info!("Successfully remuxed HEVC+RPU with all streams!");
        Ok(())
    }
}
