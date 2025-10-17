//! Formatter module for custom log formatting

pub mod filters;
pub mod levels;
pub mod styling;

use chrono::Local;
use console::style;
use std::fmt::{self as std_fmt, Debug};
use tracing::Level;
use tracing_subscriber::fmt::{format::Writer, FmtContext, FormatEvent, FormatFields};

use crate::utils::logging::text_utils;
use filters::should_show_message;
use levels::{determine_processing_level, ProcessingLevel};
use styling::{format_level, get_tree_prefix, style_message};

pub struct CleanFormatter {
    show_timestamps: bool,
    use_color: bool,
}

impl CleanFormatter {
    pub fn new(show_timestamps: bool, use_color: bool) -> Self {
        Self {
            show_timestamps,
            use_color,
        }
    }

    fn format_message(&self, message: &str, metadata_level: &Level) -> String {
        let level = determine_processing_level(message);
        let prefix = get_tree_prefix(level);

        // Get level indicator string (for WARN/ERROR)
        let level_indicator = format_level(metadata_level, self.use_color);
        let level_indicator_width = if level_indicator.is_empty() {
            0
        } else {
            level_indicator.len() + 2
        }; // +2 for spaces around it

        // Calculate available width more accurately
        // Timestamp: "[HH:MM:SS] " = 11 chars
        // Level: "WARN  " or "" = 0-7 chars (with spacing)
        // Prefix: "▶ ", "● ", or "  " = 2 chars
        let timestamp_width = if self.show_timestamps { 11 } else { 0 };
        let prefix_width = 2; // "▶ " or "● " or "  "
        let available_width = 140usize
            .saturating_sub(timestamp_width + prefix_width + level_indicator_width + 4); // 4 chars buffer

        // Clean up and format the message based on its type
        let formatted_content = match level {
            ProcessingLevel::Stage => {
                // Clean up stage messages
                let clean_message = if message.starts_with("Starting")
                    && message.contains("encoding")
                    && !message.contains("pre-encoding")
                    && !message.contains("post-encoding")
                {
                    message.replace("Starting ", "")
                } else if message.contains("CONTENT DETECTED") {
                    message.replace(" CONTENT DETECTED", " content detected")
                } else {
                    message.to_string()
                };
                style_message(&clean_message, level, self.use_color)
            }
            ProcessingLevel::Step => {
                // Summarize stream filtering results more concisely
                let clean_message =
                    if message.contains("Stream filtering") && message.contains("complete") {
                        if let Some(summary) = extract_stream_summary(message) {
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
                style_message(&clean_message, level, self.use_color)
            }
            _ => style_message(message, level, self.use_color),
        };

        // Apply text wrapping to the formatted content
        let wrapped_content = text_utils::wrap_text(&formatted_content, available_width);

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
            let continuation_indent =
                " ".repeat(timestamp_width + prefix_width + level_indicator_width);
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
        if !should_show_message(&message) {
            return Ok(());
        }

        let mut output = String::new();

        // Add timestamp if enabled (but use shorter format)
        if self.show_timestamps {
            let now = Local::now();
            let timestamp = if self.use_color {
                style(now.format("%H:%M:%S").to_string())
                    .dim()
                    .to_string()
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

/// Extracts a summary from stream filtering messages
fn extract_stream_summary(message: &str) -> Option<String> {
    // Extract key numbers from "Stream filtering with profile 'english_only' complete: 1 video, 1 audio (filtered from 2), 0 subtitle (filtered from 1), 0 data, 20 chapters"
    if let Some(colon_pos) = message.find(": ") {
        let summary_part = &message[colon_pos + 2..];
        Some(format!("Streams selected: {}", summary_part))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_stream_summary() {
        let msg = "Stream filtering with profile 'english_only' complete: 1 video, 1 audio (filtered from 2), 0 subtitle";
        let result = extract_stream_summary(msg);
        assert!(result.is_some());
        assert!(result.unwrap().contains("Streams selected:"));
    }

    #[test]
    fn test_extract_stream_summary_no_colon() {
        let msg = "Some other message";
        let result = extract_stream_summary(msg);
        assert!(result.is_none());
    }
}
