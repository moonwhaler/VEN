//! Text wrapping utilities for log formatting

/// Wraps text to fit within a maximum width, preserving special prefixes like "  -> "
pub fn wrap_text(text: &str, max_width: usize) -> String {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wrap_short_text() {
        let text = "Short text";
        let result = wrap_text(text, 80);
        assert_eq!(result, "Short text");
    }

    #[test]
    fn test_wrap_long_text() {
        let text = "This is a very long line that should be wrapped because it exceeds the maximum width that we have specified for this test";
        let result = wrap_text(text, 40);
        assert!(result.contains('\n'));
        for line in result.lines() {
            assert!(line.len() <= 40);
        }
    }

    #[test]
    fn test_wrap_with_prefix() {
        let text = "  -> This is a very long parameter line that should be wrapped while preserving the prefix";
        let result = wrap_text(text, 40);
        assert!(result.starts_with("  -> "));
        assert!(result.contains('\n'));
    }

    #[test]
    fn test_wrap_multiline() {
        let text = "Line one\nLine two that is very long and should be wrapped to fit within the maximum width";
        let result = wrap_text(text, 40);
        let lines: Vec<&str> = result.lines().collect();
        assert!(lines.len() > 2);
    }
}
