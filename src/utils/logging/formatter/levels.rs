/// Processing level determination for hierarchical log output

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcessingLevel {
    Root,   // Top level operations
    Stage,  // Major processing stages
    Step,   // Individual steps within stages
    Detail, // Detailed information
}

/// Determines the processing level of a log message based on its content
pub fn determine_processing_level(message: &str) -> ProcessingLevel {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_root_level() {
        assert_eq!(
            determine_processing_level("Processing file: test.mp4"),
            ProcessingLevel::Root
        );
        assert_eq!(
            determine_processing_level("Found 5 file(s) to process"),
            ProcessingLevel::Root
        );
    }

    #[test]
    fn test_stage_level() {
        assert_eq!(
            determine_processing_level("HDR10 CONTENT DETECTED"),
            ProcessingLevel::Stage
        );
        assert_eq!(
            determine_processing_level("Starting CRF encoding"),
            ProcessingLevel::Stage
        );
        assert_eq!(
            determine_processing_level("Crop detection completed"),
            ProcessingLevel::Stage
        );
        assert_eq!(
            determine_processing_level("PARAMETER ADJUSTMENTS applied"),
            ProcessingLevel::Stage
        );
    }

    #[test]
    fn test_step_level() {
        assert_eq!(
            determine_processing_level("Analyzing stream structure"),
            ProcessingLevel::Step
        );
        assert_eq!(
            determine_processing_level("Getting video metadata"),
            ProcessingLevel::Step
        );
        assert_eq!(
            determine_processing_level("Processing HDR10+ content"),
            ProcessingLevel::Step
        );
        assert_eq!(
            determine_processing_level("Extracting RPU metadata"),
            ProcessingLevel::Step
        );
    }

    #[test]
    fn test_detail_level() {
        assert_eq!(
            determine_processing_level("Some detail information"),
            ProcessingLevel::Detail
        );
        assert_eq!(
            determine_processing_level("Debug output line"),
            ProcessingLevel::Detail
        );
    }
}
