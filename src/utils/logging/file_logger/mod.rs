//! File logger for detailed encoding logs

pub mod analysis;
pub mod crop;
pub mod encoding;

use std::fs::File;
use std::io::BufWriter;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

pub struct FileLogger {
    writer: Arc<Mutex<BufWriter<File>>>,
    log_path: PathBuf,
}

impl FileLogger {
    pub fn new<P: AsRef<Path>>(output_path: P) -> crate::utils::Result<Self> {
        let output_path = output_path.as_ref();
        let log_path = output_path.with_extension("log");

        let file = File::create(&log_path)?;
        let writer = Arc::new(Mutex::new(BufWriter::new(file)));

        Ok(Self { writer, log_path })
    }

    #[allow(clippy::too_many_arguments)]
    pub fn log_encoding_settings(
        &self,
        input_path: &Path,
        output_path: &Path,
        profile_name: &str,
        profile_settings: &crate::config::EncodingProfile,
        mode: &str,
        adaptive_crf: f32,
        adaptive_bitrate: u32,
        filter_chain: Option<&str>,
        stream_mapping: &str,
    ) -> crate::utils::Result<()> {
        let mut writer = self.writer.lock().unwrap();
        encoding::log_encoding_settings(
            &mut *writer,
            input_path,
            output_path,
            profile_name,
            profile_settings,
            mode,
            adaptive_crf,
            adaptive_bitrate,
            filter_chain,
            stream_mapping,
        )
    }

    pub fn log_analysis_results(
        &self,
        metadata: &crate::utils::ffmpeg::VideoMetadata,
        grain_level: Option<u8>,
        content_analysis: Option<&crate::content_manager::ContentAnalysisResult>,
    ) -> crate::utils::Result<()> {
        let mut writer = self.writer.lock().unwrap();
        analysis::log_analysis_results(&mut *writer, metadata, grain_level, content_analysis)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn log_crop_detection_results(
        &self,
        enabled: bool,
        sample_count: u32,
        sample_timestamps: &[f64],
        crop_result: Option<&str>,
        detection_method: &str,
        sdr_limit: u32,
        hdr_limit: u32,
        is_hdr: bool,
    ) -> crate::utils::Result<()> {
        let mut writer = self.writer.lock().unwrap();
        crop::log_crop_detection_results(
            &mut *writer,
            enabled,
            sample_count,
            sample_timestamps,
            crop_result,
            detection_method,
            sdr_limit,
            hdr_limit,
            is_hdr,
        )
    }

    pub fn log_encoding_progress(&self, message: &str) -> crate::utils::Result<()> {
        let mut writer = self.writer.lock().unwrap();
        encoding::log_encoding_progress(&mut *writer, message)
    }

    pub fn log_encoding_complete(
        &self,
        success: bool,
        duration: std::time::Duration,
        output_size: Option<u64>,
        exit_code: Option<i32>,
    ) -> crate::utils::Result<()> {
        let mut writer = self.writer.lock().unwrap();
        encoding::log_encoding_complete(&mut *writer, success, duration, output_size, exit_code)
    }

    pub fn log_ffmpeg_command(
        &self,
        ffmpeg_path: &str,
        args: &[String],
    ) -> crate::utils::Result<()> {
        let mut writer = self.writer.lock().unwrap();
        encoding::log_ffmpeg_command(&mut *writer, ffmpeg_path, args)
    }

    pub fn get_log_path(&self) -> &Path {
        &self.log_path
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_file_logger_creation() {
        let temp_dir = std::env::temp_dir();
        let output_path = temp_dir.join("test_output.mp4");

        let logger = FileLogger::new(&output_path);
        assert!(logger.is_ok());

        let logger = logger.unwrap();
        assert_eq!(logger.get_log_path(), temp_dir.join("test_output.log"));

        // Clean up
        let _ = std::fs::remove_file(logger.get_log_path());
    }

    #[test]
    fn test_log_encoding_progress() {
        let temp_dir = std::env::temp_dir();
        let output_path = temp_dir.join("test_progress.mp4");

        let logger = FileLogger::new(&output_path).unwrap();
        let result = logger.log_encoding_progress("Test progress message");
        assert!(result.is_ok());

        // Clean up
        let _ = std::fs::remove_file(logger.get_log_path());
    }
}
