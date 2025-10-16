//! Styling and formatting for log messages

use console::style;
use tracing::Level;

use super::levels::ProcessingLevel;

/// Formats a log level with appropriate styling
pub fn format_level(level: &Level, use_color: bool) -> String {
    if !use_color {
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

/// Gets the tree prefix symbol for a given processing level
pub fn get_tree_prefix(level: ProcessingLevel) -> &'static str {
    match level {
        ProcessingLevel::Root => "▶",
        ProcessingLevel::Stage => "●",
        ProcessingLevel::Step => " ",
        ProcessingLevel::Detail => " ",
    }
}

/// Applies styling to message content based on processing level
pub fn style_message(message: &str, level: ProcessingLevel, use_color: bool) -> String {
    match level {
        ProcessingLevel::Root => {
            if use_color {
                style(message).bold().cyan().to_string()
            } else {
                message.to_uppercase()
            }
        }
        ProcessingLevel::Stage => {
            if use_color {
                style(message).bold().green().to_string()
            } else {
                message.to_string()
            }
        }
        ProcessingLevel::Step => {
            if use_color {
                style(message).cyan().to_string()
            } else {
                message.to_string()
            }
        }
        ProcessingLevel::Detail => {
            if use_color {
                style(message).dim().to_string()
            } else {
                message.to_string()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_level_no_color() {
        assert_eq!(format_level(&Level::ERROR, false), "ERROR");
        assert_eq!(format_level(&Level::WARN, false), "WARN ");
        assert_eq!(format_level(&Level::INFO, false), "");
        assert_eq!(format_level(&Level::DEBUG, false), "DEBUG");
    }

    #[test]
    fn test_format_level_with_color() {
        // With color, the strings will contain ANSI codes
        let result = format_level(&Level::ERROR, true);
        assert!(result.contains("ERROR"));

        let result = format_level(&Level::INFO, true);
        assert_eq!(result, "");
    }

    #[test]
    fn test_get_tree_prefix() {
        assert_eq!(get_tree_prefix(ProcessingLevel::Root), "▶");
        assert_eq!(get_tree_prefix(ProcessingLevel::Stage), "●");
        assert_eq!(get_tree_prefix(ProcessingLevel::Step), " ");
        assert_eq!(get_tree_prefix(ProcessingLevel::Detail), " ");
    }

    #[test]
    fn test_style_message_no_color() {
        let msg = "Test message";
        assert_eq!(
            style_message(msg, ProcessingLevel::Root, false),
            "TEST MESSAGE"
        );
        assert_eq!(style_message(msg, ProcessingLevel::Stage, false), msg);
        assert_eq!(style_message(msg, ProcessingLevel::Step, false), msg);
        assert_eq!(style_message(msg, ProcessingLevel::Detail, false), msg);
    }

    #[test]
    fn test_style_message_with_color() {
        let msg = "Test message";
        // With color, the strings will contain ANSI codes
        assert!(style_message(msg, ProcessingLevel::Root, true).contains("Test message"));
        assert!(style_message(msg, ProcessingLevel::Stage, true).contains("Test message"));
    }
}
