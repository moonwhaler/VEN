use chrono::Utc;
use console::style;
use std::fmt::{self as std_fmt, Debug};
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tracing::Level;
use tracing_subscriber::{
    fmt::{self, format::Writer, FmtContext, FormatEvent, FormatFields},
    layer::SubscriberExt,
    util::SubscriberInitExt,
    EnvFilter,
};

struct CleanFormatter {
    show_timestamps: bool,
    use_color: bool,
}

#[derive(Debug, Clone, Copy)]
enum ProcessingLevel {
    Root,   // Top level operations
    Stage,  // Major processing stages
    Step,   // Individual steps within stages
    Detail, // Detailed information
}

impl CleanFormatter {
    fn new(show_timestamps: bool, use_color: bool) -> Self {
        Self {
            show_timestamps,
            use_color,
        }
    }

    fn wrap_text(&self, text: &str, max_width: usize) -> String {
        let lines: Vec<&str> = text.lines().collect();
        let mut wrapped_lines = Vec::new();

        for line in lines {
            if line.len() <= max_width {
                wrapped_lines.push(line.to_string());
            } else {
                // Check if this is a parameter line starting with "  -> "
                let (prefix, content) = if let Some(stripped) = line.strip_prefix("  -> ") {
                    ("  -> ", stripped)
                } else {
                    ("", line)
                };

                // Split long lines at word boundaries
                let mut current_line = String::new();
                let words: Vec<&str> = content.split_whitespace().collect();
                let mut first_line = true;

                for word in &words {
                    // If adding this word would exceed the limit
                    let line_with_prefix_len = if first_line {
                        prefix.len()
                            + current_line.len()
                            + word.len()
                            + if current_line.is_empty() { 0 } else { 1 }
                    } else {
                        current_line.len()
                            + word.len()
                            + if current_line.is_empty() { 0 } else { 1 }
                    };

                    if !current_line.is_empty() && line_with_prefix_len > max_width {
                        // Push the current line and start a new one
                        if first_line {
                            wrapped_lines.push(format!("{}{}", prefix, current_line));
                            first_line = false;
                        } else {
                            wrapped_lines.push(current_line);
                        }
                        current_line = word.to_string();
                    } else {
                        // Add word to current line
                        if !current_line.is_empty() {
                            current_line.push(' ');
                        }
                        current_line.push_str(word);
                    }
                }

                // Don't forget the last line
                if !current_line.is_empty() {
                    if first_line && !prefix.is_empty() {
                        wrapped_lines.push(format!("{}{}", prefix, current_line));
                    } else {
                        wrapped_lines.push(current_line);
                    }
                }
            }
        }

        wrapped_lines.join("\n")
    }

    fn format_level(&self, level: &Level) -> String {
        if !self.use_color {
            match *level {
                Level::ERROR => "ERROR".to_string(),
                Level::WARN => "WARN ".to_string(),
                Level::INFO => "".to_string(), // Hide INFO prefix for cleaner output
                Level::DEBUG => "DEBUG".to_string(),
                Level::TRACE => "TRACE".to_string(),
            }
        } else {
            match *level {
                Level::ERROR => style("ERROR").red().bold().to_string(),
                Level::WARN => style("WARN ").yellow().to_string(),
                Level::INFO => "".to_string(), // Hide INFO prefix for cleaner output
                Level::DEBUG => style("DEBUG").blue().to_string(),
                Level::TRACE => style("TRACE").magenta().to_string(),
            }
        }
    }

    fn should_show_message(&self, message: &str) -> bool {
        // Filter out noisy FFmpeg messages that don't add value
        let noise_patterns = [
            "Invalid Block Addition value",
            "Could not find codec parameters for stream",
            "Consider increasing the value for the 'analyzeduration'",
            "x265 [info]: HEVC encoder version",
            "x265 [info]: build info",
            "x265 [info]: using cpu capabilities",
            "x265 [info]: Thread pool created",
            "x265 [info]: Slices",
            "x265 [info]: frame threads",
            "x265 [info]: Coding QT",
            "x265 [info]: Residual QT",
            "x265 [info]: ME / range",
            "x265 [info]: Keyframe min",
            "x265 [info]: Lookahead",
            "x265 [info]: b-pyramid",
            "x265 [info]: References",
            "x265 [info]: AQ:",
            "x265 [info]: Rate Control",
            "x265 [info]: tools:",
            "matroska,webm",
        ];

        !noise_patterns
            .iter()
            .any(|pattern| message.contains(pattern))
    }

    fn determine_processing_level(&self, message: &str) -> ProcessingLevel {
        // Root level - main operations
        if message.contains("Processing file")
            || (message.contains("Found") && message.contains("file(s) to process"))
        {
            return ProcessingLevel::Root;
        }

        // Stage level - major processing phases
        // Content detection results
        if message.contains("CONTENT DETECTED")
            || message.contains("Recommended encoding approach")
            || message.contains("DUAL FORMAT CONTENT DETECTED")
        {
            return ProcessingLevel::Stage;
        }

        // Major workflow phases - Starting/Initializing
        if (message.contains("Starting") || message.contains("Initializing"))
            && (message.contains("encoding")
                || message.contains("CRF")
                || message.contains("ABR")
                || message.contains("CBR")
                || message.contains("crop detection")
                || message.contains("unified content analysis")
                || message.contains("pre-encoding metadata extraction")
                || message.contains("post-encoding metadata injection")
                || message.contains("metadata workflow"))
        {
            return ProcessingLevel::Stage;
        }

        // Completion of major phases
        if message.contains("Crop detection completed")
            || message.contains("Metadata extraction phase completed")
            || message.contains("Encoding completed successfully")
        {
            return ProcessingLevel::Stage;
        }

        // Parameter adjustments (major decision point)
        if message.contains("PARAMETER ADJUSTMENTS")
            || message.contains("Using standard encoding parameters")
        {
            return ProcessingLevel::Stage;
        }

        // Step level - individual processing steps
        // Metadata and tool operations
        if message.contains("Checking external metadata tool availability")
            || message.contains("External metadata tools are ready")
            || message.contains("HDR/DV metadata tools ready")
            || message.contains("No external tools available")
            || message.contains("External metadata parameters ready")
        {
            return ProcessingLevel::Step;
        }

        // Stream operations
        if message.contains("Analyzing stream structure")
            || (message.contains("Stream") && message.contains("complete"))
            || message.contains("Stream analysis complete")
            || message.contains("Stream filtering")
        {
            return ProcessingLevel::Step;
        }

        // Video analysis and metadata
        if message.contains("Getting video metadata")
            || message.contains("Analyzing video metadata")
        {
            return ProcessingLevel::Step;
        }

        // Profile selection
        if message.contains("Auto-selecting profile")
            || message.contains("Selected profile based on")
            || message.contains("No specific profile found")
        {
            return ProcessingLevel::Step;
        }

        // Content processing substeps
        if message.contains("Processing") &&
            (message.contains("SDR content")
                || message.contains("HDR10+ content")
                || message.contains("standard HDR10 content")
                || message.contains("Dolby Vision content")
                || message.contains("dual format content"))
        {
            return ProcessingLevel::Step;
        }

        // Metadata extraction/injection operations
        if message.contains("Extracting") &&
            (message.contains("RPU metadata")
                || message.contains("HDR10+ dynamic metadata")
                || message.contains("HDR10+ metadata"))
        {
            return ProcessingLevel::Step;
        }

        if message.contains("Injecting") &&
            (message.contains("RPU metadata")
                || message.contains("Dolby Vision"))
        {
            return ProcessingLevel::Step;
        }

        // Extraction/injection results
        if (message.contains("extraction successful")
                || message.contains("injection successful")
                || message.contains("No external metadata extracted"))
            && !message.contains("  ")  // Not indented detail messages
        {
            return ProcessingLevel::Step;
        }

        // x265 parameter information
        if message.contains("x265 parameters injected")
            || (message.contains("x265 parameters") && message.contains("injected"))
        {
            return ProcessingLevel::Step;
        }

        // HDR10+ processing substeps
        if message.contains("HDR10+ metadata was successfully included")
            || message.contains("HDR10+ metadata was included during")
        {
            return ProcessingLevel::Step;
        }

        // Skipping operations (important decision points)
        if message.contains("Skipping") &&
            (message.contains("RPU extraction")
                || message.contains("HDR10+ metadata extraction"))
        {
            return ProcessingLevel::Step;
        }

        // Detail level - supporting information
        ProcessingLevel::Detail
    }

    fn get_tree_prefix(&self, level: ProcessingLevel) -> &'static str {
        match level {
            ProcessingLevel::Root => "▶",
            ProcessingLevel::Stage => "●",
            ProcessingLevel::Step => " ",
            ProcessingLevel::Detail => " ",
        }
    }

    fn format_message(&self, message: &str, metadata_level: &Level) -> String {
        let level = self.determine_processing_level(message);
        let prefix = self.get_tree_prefix(level);

        // Get level indicator string (for WARN/ERROR)
        let level_indicator = self.format_level(metadata_level);
        let level_indicator_width = if level_indicator.is_empty() { 0 } else { level_indicator.len() + 2 }; // +2 for spaces around it

        // Calculate available width more accurately
        // Timestamp: "[HH:MM:SS] " = 11 chars
        // Level: "WARN  " or "" = 0-7 chars (with spacing)
        // Prefix: "▶ ", "● ", or "  " = 2 chars
        let timestamp_width = if self.show_timestamps { 11 } else { 0 };
        let prefix_width = 2; // "▶ " or "● " or "  "
        let available_width = 140usize.saturating_sub(timestamp_width + prefix_width + level_indicator_width + 4); // 4 chars buffer

        // Clean up and format the message based on its type
        let formatted_content = match level {
            ProcessingLevel::Root => {
                if self.use_color {
                    style(message).bold().cyan().to_string()
                } else {
                    message.to_uppercase()
                }
            }
            ProcessingLevel::Stage => {
                // Clean up stage messages
                let clean_message = if message.starts_with("Starting") && message.contains("encoding")
                    && !message.contains("pre-encoding") && !message.contains("post-encoding")
                {
                    message.replace("Starting ", "")
                } else if message.contains("CONTENT DETECTED") {
                    message.replace(" CONTENT DETECTED", " content detected")
                } else {
                    message.to_string()
                };

                if self.use_color {
                    style(clean_message).bold().green().to_string()
                } else {
                    clean_message
                }
            }
            ProcessingLevel::Step => {
                // Summarize stream filtering results more concisely
                let clean_message =
                    if message.contains("Stream filtering") && message.contains("complete") {
                        if let Some(summary) = self.extract_stream_summary(message) {
                            summary
                        } else {
                            message.to_string()
                        }
                    } else if message.contains("External metadata tools are ready") {
                        "HDR/DV metadata tools ready".to_string()
                    } else if message.contains("Getting video metadata for:") {
                        "Analyzing video metadata".to_string()
                    } else {
                        message.to_string()
                    };

                if self.use_color {
                    style(clean_message).cyan().to_string()
                } else {
                    clean_message
                }
            }
            ProcessingLevel::Detail => {
                if self.use_color {
                    style(message).dim().to_string()
                } else {
                    message.to_string()
                }
            }
        };

        // Apply text wrapping to the formatted content
        let wrapped_content = self.wrap_text(&formatted_content, available_width);

        // Build the line with level indicator (if present) after the prefix
        let level_prefix = if !level_indicator.is_empty() {
            format!("{} ", level_indicator)
        } else {
            String::new()
        };

        // Handle multi-line wrapped content
        if wrapped_content.contains('\n') {
            let lines: Vec<&str> = wrapped_content.lines().collect();
            let first_line = format!("{} {}{}", prefix, level_prefix, lines[0]);

            // Calculate the appropriate indentation for continuation lines
            // Must account for prefix + level indicator
            let continuation_indent = " ".repeat(timestamp_width + prefix_width + level_indicator_width);
            let continuation_lines: Vec<String> = lines[1..]
                .iter()
                .map(|line| format!("{}{}", continuation_indent, line))
                .collect();

            if continuation_lines.is_empty() {
                first_line
            } else {
                format!("{}\n{}", first_line, continuation_lines.join("\n"))
            }
        } else {
            format!("{} {}{}", prefix, level_prefix, wrapped_content)
        }
    }

    fn extract_stream_summary(&self, message: &str) -> Option<String> {
        // Extract key numbers from "Stream filtering with profile 'english_only' complete: 1 video, 1 audio (filtered from 2), 0 subtitle (filtered from 1), 0 data, 20 chapters"
        if let Some(colon_pos) = message.find(": ") {
            let summary_part = &message[colon_pos + 2..];
            Some(format!("Streams selected: {}", summary_part))
        } else {
            None
        }
    }
}

impl<S, N> FormatEvent<S, N> for CleanFormatter
where
    S: tracing::Subscriber + for<'a> tracing_subscriber::registry::LookupSpan<'a>,
    N: for<'a> FormatFields<'a> + 'static,
{
    fn format_event(
        &self,
        _ctx: &FmtContext<'_, S, N>,
        mut writer: Writer<'_>,
        event: &tracing::Event<'_>,
    ) -> std_fmt::Result {
        let metadata = event.metadata();
        let message = {
            let mut visitor = MessageVisitor::default();
            event.record(&mut visitor);
            visitor.message
        };

        // Filter out noisy messages
        if !self.should_show_message(&message) {
            return Ok(());
        }

        let mut output = String::new();

        // Add timestamp if enabled (but use shorter format)
        if self.show_timestamps {
            let now = chrono::Utc::now();
            let timestamp = if self.use_color {
                style(now.format("%H:%M:%S").to_string()).dim().to_string()
            } else {
                now.format("%H:%M:%S").to_string()
            };
            output.push_str(&format!("[{}] ", timestamp));
        }

        // Add formatted message (which now includes the level indicator in the appropriate position)
        output.push_str(&self.format_message(&message, metadata.level()));

        writeln!(writer, "{}", output)
    }
}

#[derive(Default)]
struct MessageVisitor {
    message: String,
}

impl tracing::field::Visit for MessageVisitor {
    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn Debug) {
        if field.name() == "message" {
            self.message = format!("{:?}", value).trim_matches('"').to_string();
        }
    }
}

pub fn setup_logging(
    level: &str,
    show_timestamps: bool,
    colored: bool,
) -> crate::utils::Result<()> {
    let level = match level.to_lowercase().as_str() {
        "trace" => Level::TRACE,
        "debug" => Level::DEBUG,
        "info" => Level::INFO,
        "warn" => Level::WARN,
        "error" => Level::ERROR,
        _ => Level::INFO,
    };

    let env_filter = EnvFilter::builder()
        .with_default_directive(level.into())
        .from_env_lossy();

    // Use our clean formatter for better console output
    let formatter = CleanFormatter::new(show_timestamps, colored);
    let fmt_layer = fmt::layer()
        .with_target(false)
        .with_level(false) // We handle level formatting in our custom formatter
        .event_format(formatter);

    tracing_subscriber::registry()
        .with(env_filter)
        .with(fmt_layer)
        .init();

    Ok(())
}

pub fn log_encoding_start(input: &str, output: &str, profile: &str, mode: &str) {
    tracing::info!(
        "Starting encoding: {} -> {} (profile: {}, mode: {})",
        input,
        output,
        profile,
        mode
    );
}

pub fn log_encoding_complete(duration: std::time::Duration, output_size: u64) {
    tracing::info!(
        "Encoding completed in {:.2}s, output size: {:.2} MB",
        duration.as_secs_f64(),
        output_size as f64 / 1_048_576.0
    );
}

pub fn log_analysis_result(content_type: &str, grain_level: u8) {
    tracing::info!("Analysis: type={}, grain={}", content_type, grain_level);
}

pub fn log_crop_detection(crop_values: &str) {
    tracing::info!("Crop detected: {}", crop_values);
}

pub fn log_profile_selection(profile: &str, reason: &str) {
    tracing::info!("Profile selected: {} ({})", profile, reason);
}

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
        let timestamp = Utc::now().format("%Y-%m-%d %H:%M:%S UTC");

        let mut writer = self.writer.lock().unwrap();

        writeln!(writer, "FFmpeg Encoder - Encoding Log")?;
        writeln!(writer, "Generated: {}", timestamp)?;
        writeln!(writer, "========================================\n")?;

        writeln!(writer, "INPUT/OUTPUT:")?;
        writeln!(writer, "  Input:  {}", input_path.display())?;
        writeln!(writer, "  Output: {}", output_path.display())?;
        writeln!(writer)?;

        writeln!(writer, "ENCODING SETTINGS:")?;
        writeln!(writer, "  Mode: {}", mode.to_uppercase())?;
        writeln!(
            writer,
            "  Profile: {} - {}",
            profile_name, profile_settings.title
        )?;
        writeln!(
            writer,
            "  Content Type: {:?}",
            profile_settings.content_type
        )?;
        writeln!(writer, "  Adaptive CRF: {}", adaptive_crf)?;
        writeln!(writer, "  Adaptive Bitrate: {} kbps", adaptive_bitrate)?;
        writeln!(writer)?;

        if let Some(filters) = filter_chain {
            writeln!(writer, "VIDEO FILTERS:")?;
            writeln!(writer, "  {}", filters)?;
            writeln!(writer)?;
        }

        writeln!(writer, "STREAM MAPPING:")?;
        writeln!(writer, "  {}", stream_mapping)?;
        writeln!(writer)?;

        writeln!(writer, "x265 PARAMETERS:")?;
        for (key, value) in &profile_settings.x265_params {
            writeln!(writer, "  {}: {}", key, value)?;
        }
        writeln!(writer)?;

        writer.flush()?;
        Ok(())
    }

    pub fn log_analysis_results(
        &self,
        metadata: &crate::utils::ffmpeg::VideoMetadata,
        grain_level: Option<u8>,
        content_analysis: Option<&crate::content_manager::ContentAnalysisResult>,
    ) -> crate::utils::Result<()> {
        let mut writer = self.writer.lock().unwrap();

        writeln!(writer, "VIDEO ANALYSIS:")?;
        writeln!(
            writer,
            "  Resolution: {}x{}",
            metadata.width, metadata.height
        )?;
        writeln!(writer, "  Duration: {:.2}s", metadata.duration)?;
        writeln!(writer, "  Framerate: {:.2} fps", metadata.fps)?;
        writeln!(
            writer,
            "  Codec: {}",
            metadata.codec.as_deref().unwrap_or("Unknown")
        )?;
        if let Some(bitrate) = metadata.bitrate {
            writeln!(writer, "  Bitrate: {} kbps", bitrate)?;
        }
        writeln!(
            writer,
            "  HDR: {}",
            if metadata.is_hdr { "Yes" } else { "No" }
        )?;

        if let Some(grain) = grain_level {
            writeln!(writer, "  Grain Level: {}", grain)?;
        }

        // Enhanced HDR/DV analysis logging
        if let Some(analysis) = content_analysis {
            writeln!(writer)?;
            writeln!(writer, "HDR/DOLBY VISION ANALYSIS:")?;

            match &analysis.recommended_approach {
                crate::content_manager::ContentEncodingApproach::SDR => {
                    writeln!(writer, "  Content Type: SDR (Standard Dynamic Range)")?;
                    writeln!(writer, "  HDR Format: None")?;
                }
                crate::content_manager::ContentEncodingApproach::HDR(hdr_result) => {
                    writeln!(writer, "  Content Type: HDR (High Dynamic Range)")?;
                    writeln!(writer, "  HDR Format: {:?}", hdr_result.metadata.format)?;
                    writeln!(
                        writer,
                        "  Detection Confidence: {:.1}%",
                        hdr_result.confidence_score * 100.0
                    )?;

                    // Color space information
                    if let Some(ref cs) = hdr_result.metadata.raw_color_space {
                        writeln!(writer, "  Color Space: {}", cs)?;
                    }
                    if let Some(ref tf) = hdr_result.metadata.raw_transfer {
                        writeln!(writer, "  Transfer Function: {}", tf)?;
                    }
                    if let Some(ref cp) = hdr_result.metadata.raw_primaries {
                        writeln!(writer, "  Color Primaries: {}", cp)?;
                    }

                    // Mastering display metadata
                    if let Some(ref master_display) = hdr_result.metadata.master_display {
                        writeln!(writer, "  Mastering Display Metadata:")?;
                        writeln!(
                            writer,
                            "    Red Primary: ({:.4}, {:.4})",
                            master_display.red_primary.0, master_display.red_primary.1
                        )?;
                        writeln!(
                            writer,
                            "    Green Primary: ({:.4}, {:.4})",
                            master_display.green_primary.0, master_display.green_primary.1
                        )?;
                        writeln!(
                            writer,
                            "    Blue Primary: ({:.4}, {:.4})",
                            master_display.blue_primary.0, master_display.blue_primary.1
                        )?;
                        writeln!(
                            writer,
                            "    White Point: ({:.4}, {:.4})",
                            master_display.white_point.0, master_display.white_point.1
                        )?;
                        writeln!(
                            writer,
                            "    Max Luminance: {} nits",
                            master_display.max_luminance
                        )?;
                        writeln!(
                            writer,
                            "    Min Luminance: {:.4} nits",
                            master_display.min_luminance
                        )?;
                    }

                    // Content light level information
                    if let Some(ref cll) = hdr_result.metadata.content_light_level {
                        writeln!(writer, "  Content Light Level:")?;
                        writeln!(writer, "    Max CLL: {} nits", cll.max_cll)?;
                        writeln!(writer, "    Max FALL: {} nits", cll.max_fall)?;
                    }
                }
                crate::content_manager::ContentEncodingApproach::DolbyVision(dv_info) => {
                    writeln!(writer, "  Content Type: Dolby Vision")?;
                    writeln!(
                        writer,
                        "  Dolby Vision Profile: {}",
                        dv_info.profile.as_str()
                    )?;
                    writeln!(
                        writer,
                        "  Profile Description: {}",
                        Self::get_dv_profile_description(&dv_info.profile)
                    )?;
                    writeln!(
                        writer,
                        "  RPU Present: {}",
                        if dv_info.rpu_present { "Yes" } else { "No" }
                    )?;
                    writeln!(
                        writer,
                        "  Has Enhancement Layer: {}",
                        if dv_info.has_enhancement_layer {
                            "Yes"
                        } else {
                            "No"
                        }
                    )?;
                    writeln!(
                        writer,
                        "  EL Present: {}",
                        if dv_info.el_present { "Yes" } else { "No" }
                    )?;
                    writeln!(
                        writer,
                        "  HDR10 Compatible: {}",
                        if dv_info.profile.supports_hdr10_compatibility() {
                            "Yes"
                        } else {
                            "No"
                        }
                    )?;
                    writeln!(
                        writer,
                        "  Dual Layer: {}",
                        if dv_info.profile.is_dual_layer() {
                            "Yes"
                        } else {
                            "No"
                        }
                    )?;

                    if let Some(bl_compatible_id) = dv_info.bl_compatible_id {
                        writeln!(writer, "  BL Compatible ID: {}", bl_compatible_id)?;
                    }

                    if let Some(ref codec_profile) = dv_info.codec_profile {
                        writeln!(writer, "  Codec Profile: {}", codec_profile)?;
                    }
                }
                crate::content_manager::ContentEncodingApproach::DolbyVisionWithHDR10Plus(
                    dv_info,
                    hdr_result,
                ) => {
                    writeln!(
                        writer,
                        "  Content Type: Dual Format (Dolby Vision + HDR10+)"
                    )?;

                    // Dolby Vision information
                    writeln!(writer, "  Dolby Vision:")?;
                    writeln!(writer, "    Profile: {}", dv_info.profile.as_str())?;
                    writeln!(
                        writer,
                        "    Profile Description: {}",
                        Self::get_dv_profile_description(&dv_info.profile)
                    )?;
                    writeln!(
                        writer,
                        "    RPU Present: {}",
                        if dv_info.rpu_present { "Yes" } else { "No" }
                    )?;
                    writeln!(
                        writer,
                        "    HDR10 Compatible: {}",
                        if dv_info.profile.supports_hdr10_compatibility() {
                            "Yes"
                        } else {
                            "No"
                        }
                    )?;
                    writeln!(
                        writer,
                        "    EL Present: {}",
                        if dv_info.el_present { "Yes" } else { "No" }
                    )?;

                    // HDR10+ information
                    writeln!(writer, "  HDR10+ Format: {:?}", hdr_result.metadata.format)?;
                    writeln!(
                        writer,
                        "  HDR Detection Confidence: {:.1}%",
                        hdr_result.confidence_score * 100.0
                    )?;

                    if let Some(ref master_display) = hdr_result.metadata.master_display {
                        writeln!(
                            writer,
                            "  Max Luminance: {} nits",
                            master_display.max_luminance
                        )?;
                        writeln!(
                            writer,
                            "  Min Luminance: {:.4} nits",
                            master_display.min_luminance
                        )?;
                    }

                    if let Some(ref cll) = hdr_result.metadata.content_light_level {
                        writeln!(writer, "  Max CLL: {} nits", cll.max_cll)?;
                        writeln!(writer, "  Max FALL: {} nits", cll.max_fall)?;
                    }
                }
            }

            // Encoding adjustments section
            writeln!(writer)?;
            writeln!(writer, "CONTENT-BASED ENCODING ADJUSTMENTS:")?;
            writeln!(
                writer,
                "  CRF Adjustment: {:+.1}",
                analysis.encoding_adjustments.crf_adjustment
            )?;
            writeln!(
                writer,
                "  Bitrate Multiplier: {:.2}x",
                analysis.encoding_adjustments.bitrate_multiplier
            )?;
            writeln!(
                writer,
                "  Encoding Complexity: {:.2}x",
                analysis.encoding_adjustments.encoding_complexity
            )?;
            writeln!(
                writer,
                "  Recommended CRF Range: {:.1}-{:.1}",
                analysis.encoding_adjustments.recommended_crf_range.0,
                analysis.encoding_adjustments.recommended_crf_range.1
            )?;

            if analysis.encoding_adjustments.requires_vbv {
                writeln!(writer, "  VBV Required: Yes")?;
                if let Some(bufsize) = analysis.encoding_adjustments.vbv_bufsize {
                    writeln!(writer, "  VBV Buffer Size: {} kbps", bufsize)?;
                }
                if let Some(maxrate) = analysis.encoding_adjustments.vbv_maxrate {
                    writeln!(writer, "  VBV Max Rate: {} kbps", maxrate)?;
                }
            } else {
                writeln!(writer, "  VBV Required: No")?;
            }

            // HDR10+ specific information
            if let Some(ref hdr10plus_result) = analysis.hdr10_plus {
                writeln!(writer)?;
                writeln!(writer, "HDR10+ DYNAMIC METADATA:")?;
                writeln!(
                    writer,
                    "  Extraction Successful: {}",
                    if hdr10plus_result.extraction_successful {
                        "Yes"
                    } else {
                        "No"
                    }
                )?;
                writeln!(
                    writer,
                    "  Metadata File: {}",
                    hdr10plus_result.metadata_file.display()
                )?;
                writeln!(writer, "  Curve Count: {}", hdr10plus_result.curve_count)?;
                writeln!(writer, "  Scene Count: {}", hdr10plus_result.scene_count)?;

                if let Some(file_size) = hdr10plus_result.file_size {
                    writeln!(writer, "  Metadata File Size: {} bytes", file_size)?;
                }

                // Access metadata fields directly since it's not optional
                let metadata = &hdr10plus_result.metadata;
                writeln!(writer, "  Metadata Version: {}", metadata.version)?;
                writeln!(writer, "  Frame Count: {}", metadata.num_frames)?;

                if let Some(ref source) = metadata.source {
                    writeln!(
                        writer,
                        "  Source: {}",
                        source.filename.as_deref().unwrap_or("Unknown")
                    )?;
                    if let Some(resolution) = &source.resolution {
                        writeln!(writer, "  Source Resolution: {}", resolution)?;
                    }
                    if let Some(frame_rate) = source.frame_rate {
                        writeln!(writer, "  Source Frame Rate: {:.2} fps", frame_rate)?;
                    }
                }

                // Scene information
                if let Some(ref scene_info) = metadata.scene_info {
                    writeln!(writer, "  Scene Count: {}", scene_info.len())?;
                    for (i, scene) in scene_info.iter().enumerate().take(3) {
                        // Limit to first 3
                        writeln!(
                            writer,
                            "    Scene {}: Frames {}-{}, Avg MaxRGB: {:.2}",
                            i + 1,
                            scene.first_frame,
                            scene.last_frame,
                            scene.average_maxrgb.unwrap_or(0.0)
                        )?;
                    }
                    if scene_info.len() > 3 {
                        writeln!(writer, "    ... and {} more scenes", scene_info.len() - 3)?;
                    }
                }

                // Frame metadata summary
                if !metadata.frames.is_empty() {
                    writeln!(
                        writer,
                        "  Frame Metadata: {} frames with tone mapping data",
                        metadata.frames.len()
                    )?;
                    if let Some(first_frame) = metadata.frames.first() {
                        if let Some(app_version) = first_frame.application_version {
                            writeln!(writer, "  Application Version: {}", app_version)?;
                        }
                        if let Some(target_lum) =
                            first_frame.targeted_system_display_maximum_luminance
                        {
                            writeln!(writer, "  Target Max Luminance: {:.1} nits", target_lum)?;
                        }
                    }
                }
            }
        }

        writeln!(writer)?;
        writer.flush()?;
        Ok(())
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

        writeln!(writer, "CROP DETECTION:")?;
        writeln!(writer, "  Enabled: {}", if enabled { "Yes" } else { "No" })?;

        if !enabled {
            writeln!(writer)?;
            writer.flush()?;
            return Ok(());
        }

        writeln!(writer, "  Sample Count: {}", sample_count)?;

        // Format timestamps for display
        let timestamp_display =
            if sample_timestamps.len() == 1 && (sample_timestamps[0] + 1.0).abs() < f64::EPSILON {
                "Manual Override".to_string()
            } else {
                sample_timestamps
                    .iter()
                    .map(|&t| format!("{:.1}s", t))
                    .collect::<Vec<_>>()
                    .join(", ")
            };
        writeln!(writer, "  Sample Timestamps: {}", timestamp_display)?;
        writeln!(writer, "  Detection Method: {}", detection_method)?;

        let used_limit = if is_hdr { hdr_limit } else { sdr_limit };
        writeln!(
            writer,
            "  Crop Threshold: {} ({} content)",
            used_limit,
            if is_hdr { "HDR" } else { "SDR" }
        )?;

        match crop_result {
            Some(crop) => {
                writeln!(writer, "  Result: CROP DETECTED")?;
                writeln!(writer, "  Crop Values: {}", crop)?;

                // Parse and calculate crop statistics
                if let Some(stats) = self.parse_crop_statistics(crop) {
                    writeln!(writer, "  Original Resolution: {}x{}", stats.0, stats.1)?;
                    writeln!(writer, "  Cropped Resolution: {}x{}", stats.2, stats.3)?;
                    writeln!(writer, "  Pixels Removed: {:.1}%", stats.4)?;
                }
            }
            None => {
                writeln!(writer, "  Result: NO CROP DETECTED")?;
                writeln!(
                    writer,
                    "  Reason: No consistent black bars found across sample points"
                )?;
            }
        }

        writeln!(writer)?;
        writer.flush()?;
        Ok(())
    }

    fn parse_crop_statistics(&self, crop_str: &str) -> Option<(u32, u32, u32, u32, f32)> {
        // Parse crop string like "1920:800:0:140"
        let parts: Vec<&str> = crop_str.split(':').collect();
        if parts.len() != 4 {
            return None;
        }

        let width: u32 = parts[0].parse().ok()?;
        let height: u32 = parts[1].parse().ok()?;
        let _x: u32 = parts[2].parse().ok()?;
        let _y: u32 = parts[3].parse().ok()?;

        // For statistics, we need to calculate based on common resolutions
        // This is a simple heuristic - in practice, we'd pass the original resolution
        let (orig_width, orig_height) = if width <= 1920 && height <= 1080 {
            (1920u32, 1080u32)
        } else if width <= 3840 && height <= 2160 {
            (3840u32, 2160u32)
        } else {
            // Estimate based on crop dimensions
            (width, height + 280) // Common letterbox height
        };

        let orig_pixels = (orig_width * orig_height) as f32;
        let crop_pixels = (width * height) as f32;
        let removed_percent = ((orig_pixels - crop_pixels) / orig_pixels) * 100.0;

        Some((orig_width, orig_height, width, height, removed_percent))
    }

    pub fn log_encoding_progress(&self, message: &str) -> crate::utils::Result<()> {
        let mut writer = self.writer.lock().unwrap();
        let timestamp = Utc::now().format("%H:%M:%S");
        writeln!(writer, "[{}] {}", timestamp, message)?;
        writer.flush()?;
        Ok(())
    }

    pub fn log_encoding_complete(
        &self,
        success: bool,
        duration: std::time::Duration,
        output_size: Option<u64>,
        exit_code: Option<i32>,
    ) -> crate::utils::Result<()> {
        let mut writer = self.writer.lock().unwrap();

        writeln!(writer, "ENCODING RESULT:")?;
        writeln!(
            writer,
            "  Status: {}",
            if success { "SUCCESS" } else { "FAILED" }
        )?;
        writeln!(writer, "  Duration: {:.2}s", duration.as_secs_f64())?;

        if let Some(size) = output_size {
            writeln!(writer, "  Output Size: {:.2} MB", size as f64 / 1_048_576.0)?;
        }

        if let Some(code) = exit_code {
            writeln!(writer, "  Exit Code: {}", code)?;
        }

        let timestamp = Utc::now().format("%Y-%m-%d %H:%M:%S UTC");
        writeln!(writer, "  Completed: {}", timestamp)?;
        writeln!(writer)?;

        writer.flush()?;
        Ok(())
    }

    pub fn log_ffmpeg_command(
        &self,
        ffmpeg_path: &str,
        args: &[String],
    ) -> crate::utils::Result<()> {
        let mut writer = self.writer.lock().unwrap();

        writeln!(writer, "RAW FFMPEG COMMAND:")?;

        // Build the complete command exactly as it will be executed
        // start_encoding() adds -y at the beginning, so we replicate that here
        let mut full_command = vec![ffmpeg_path.to_string(), "-y".to_string()];
        full_command.extend_from_slice(args);

        // Write as a single line that can be copy-pasted
        writeln!(writer, "  {}", full_command.join(" "))?;
        writeln!(writer)?;

        writer.flush()?;
        Ok(())
    }

    pub fn get_log_path(&self) -> &Path {
        &self.log_path
    }

    /// Helper method to get Dolby Vision profile descriptions
    fn get_dv_profile_description(
        profile: &crate::analysis::dolby_vision::DolbyVisionProfile,
    ) -> &'static str {
        match profile {
            crate::analysis::dolby_vision::DolbyVisionProfile::None => "Not Dolby Vision",
            crate::analysis::dolby_vision::DolbyVisionProfile::Profile5 => "Single-layer DV only",
            crate::analysis::dolby_vision::DolbyVisionProfile::Profile7 => {
                "Dual-layer (BL + EL + RPU)"
            }
            crate::analysis::dolby_vision::DolbyVisionProfile::Profile81 => {
                "Single-layer with HDR10 compatibility"
            }
            crate::analysis::dolby_vision::DolbyVisionProfile::Profile82 => {
                "Single-layer with SDR compatibility"
            }
            crate::analysis::dolby_vision::DolbyVisionProfile::Profile84 => {
                "HDMI streaming profile"
            }
        }
    }
}
