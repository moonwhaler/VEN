use crate::config::types::{AudioSelectionConfig, StreamSelectionConfig, SubtitleSelectionConfig};
use crate::utils::{Error, FfmpegWrapper, Result};
use regex::Regex;
use serde_json::{from_str, Value};
use std::path::Path;
use tracing::{debug, info, warn};

#[derive(Debug, Clone)]
pub struct StreamInfo {
    pub index: u32,
    pub codec_type: String,
    pub codec_name: String,
    pub language: Option<String>,
    pub title: Option<String>,
    pub disposition: StreamDisposition,
}

#[derive(Debug, Clone)]
pub struct StreamDisposition {
    pub default: bool,
    pub forced: bool,
    pub comment: bool,
    pub lyrics: bool,
    pub karaoke: bool,
    pub original: bool,
    pub dub: bool,
    pub visual_impaired: bool,
    pub hearing_impaired: bool,
}

#[derive(Debug, Clone)]
pub struct ChapterInfo {
    pub id: u32,
    pub time_base: String,
    pub start: u64,
    pub start_time: f64,
    pub end: u64,
    pub end_time: f64,
    pub title: Option<String>,
}

#[derive(Debug, Clone)]
pub struct StreamMapping {
    pub video_streams: Vec<StreamInfo>,
    pub audio_streams: Vec<StreamInfo>,
    pub subtitle_streams: Vec<StreamInfo>,
    pub data_streams: Vec<StreamInfo>,
    pub chapters: Vec<ChapterInfo>,
    pub metadata: Vec<(String, String)>,
    pub mapping_args: Vec<String>,
}

pub struct StreamPreservation {
    ffmpeg: FfmpegWrapper,
}

impl StreamPreservation {
    pub fn new(ffmpeg: FfmpegWrapper) -> Self {
        Self { ffmpeg }
    }

    pub async fn analyze_streams<P: AsRef<Path>>(&self, input_path: P) -> Result<StreamMapping> {
        let input_path = input_path.as_ref();

        info!("Analyzing stream structure: {}", input_path.display());

        // Get comprehensive stream information
        let streams = self.get_stream_info(input_path).await?;
        let chapters = self.get_chapter_info(input_path).await?;
        let metadata = self.get_global_metadata(input_path).await?;

        // Categorize streams
        let video_streams: Vec<StreamInfo> = streams
            .iter()
            .filter(|s| s.codec_type == "video")
            .cloned()
            .collect();

        let audio_streams: Vec<StreamInfo> = streams
            .iter()
            .filter(|s| s.codec_type == "audio")
            .cloned()
            .collect();

        let subtitle_streams: Vec<StreamInfo> = streams
            .iter()
            .filter(|s| s.codec_type == "subtitle")
            .cloned()
            .collect();

        let data_streams: Vec<StreamInfo> = streams
            .iter()
            .filter(|s| s.codec_type == "data" || s.codec_type == "attachment")
            .cloned()
            .collect();

        // Build mapping arguments (default behavior - copy all streams)
        let mapping_args = self.build_mapping_arguments(&streams)?;

        info!(
            "Stream analysis complete: {} video, {} audio, {} subtitle, {} data, {} chapters",
            video_streams.len(),
            audio_streams.len(),
            subtitle_streams.len(),
            data_streams.len(),
            chapters.len()
        );

        Ok(StreamMapping {
            video_streams,
            audio_streams,
            subtitle_streams,
            data_streams,
            chapters,
            metadata,
            mapping_args,
        })
    }

    pub async fn analyze_streams_with_filtering<P: AsRef<Path>>(
        &self,
        input_path: P,
        stream_config: &StreamSelectionConfig,
    ) -> Result<StreamMapping> {
        let input_path = input_path.as_ref();

        info!(
            "Analyzing stream structure with filtering: {}",
            input_path.display()
        );

        // Get comprehensive stream information
        let streams = self.get_stream_info(input_path).await?;
        let chapters = self.get_chapter_info(input_path).await?;
        let metadata = self.get_global_metadata(input_path).await?;

        // Categorize streams
        let video_streams: Vec<StreamInfo> = streams
            .iter()
            .filter(|s| s.codec_type == "video")
            .cloned()
            .collect();

        let mut audio_streams: Vec<StreamInfo> = streams
            .iter()
            .filter(|s| s.codec_type == "audio")
            .cloned()
            .collect();

        let mut subtitle_streams: Vec<StreamInfo> = streams
            .iter()
            .filter(|s| s.codec_type == "subtitle")
            .cloned()
            .collect();

        let data_streams: Vec<StreamInfo> = streams
            .iter()
            .filter(|s| s.codec_type == "data" || s.codec_type == "attachment")
            .cloned()
            .collect();

        // Apply stream filtering if enabled
        if stream_config.enabled {
            audio_streams = self.filter_audio_streams(audio_streams, &stream_config.audio)?;
            subtitle_streams =
                self.filter_subtitle_streams(subtitle_streams, &stream_config.subtitle)?;
        }

        // Build mapping arguments with filtered streams
        let filtered_streams: Vec<StreamInfo> = video_streams
            .iter()
            .chain(audio_streams.iter())
            .chain(subtitle_streams.iter())
            .chain(data_streams.iter())
            .cloned()
            .collect();

        let mapping_args = if stream_config.enabled {
            self.build_filtered_mapping_arguments(
                &video_streams,
                &audio_streams,
                &subtitle_streams,
                &data_streams,
            )?
        } else {
            self.build_mapping_arguments(&filtered_streams)?
        };

        info!(
            "Stream filtering complete: {} video, {} audio (filtered from {}), {} subtitle (filtered from {}), {} data, {} chapters",
            video_streams.len(),
            audio_streams.len(),
            streams.iter().filter(|s| s.codec_type == "audio").count(),
            subtitle_streams.len(),
            streams.iter().filter(|s| s.codec_type == "subtitle").count(),
            data_streams.len(),
            chapters.len()
        );

        Ok(StreamMapping {
            video_streams,
            audio_streams,
            subtitle_streams,
            data_streams,
            chapters,
            metadata,
            mapping_args,
        })
    }

    async fn get_stream_info<P: AsRef<Path>>(&self, input_path: P) -> Result<Vec<StreamInfo>> {
        let input_path = input_path.as_ref();

        // Use the integrated FFmpeg wrapper for better performance
        debug!(
            "Using FFmpeg wrapper for stream analysis: {}",
            input_path.display()
        );

        let output = self
            .ffmpeg
            .run_ffprobe(&[
                "-v",
                "quiet",
                "-analyzeduration",
                "5M", // Optimized for faster analysis
                "-probesize",
                "5M", // Optimized for faster analysis
                "-print_format",
                "json",
                "-show_streams",
                "-show_format",
                &input_path.to_string_lossy(),
            ])
            .await?;

        let json: Value = from_str(&output)?;

        let mut streams = Vec::new();

        if let Some(stream_array) = json["streams"].as_array() {
            for (index, stream) in stream_array.iter().enumerate() {
                let codec_type = stream["codec_type"]
                    .as_str()
                    .unwrap_or("unknown")
                    .to_string();
                let codec_name = stream["codec_name"]
                    .as_str()
                    .unwrap_or("unknown")
                    .to_string();
                let language = stream["tags"]["language"].as_str().map(|s| s.to_string());
                let title = stream["tags"]["title"].as_str().map(|s| s.to_string());

                // Parse disposition
                let disposition = if let Some(disp) = stream["disposition"].as_object() {
                    StreamDisposition {
                        default: disp["default"].as_i64().unwrap_or(0) == 1,
                        forced: disp["forced"].as_i64().unwrap_or(0) == 1,
                        comment: disp["comment"].as_i64().unwrap_or(0) == 1,
                        lyrics: disp["lyrics"].as_i64().unwrap_or(0) == 1,
                        karaoke: disp["karaoke"].as_i64().unwrap_or(0) == 1,
                        original: disp["original"].as_i64().unwrap_or(0) == 1,
                        dub: disp["dub"].as_i64().unwrap_or(0) == 1,
                        visual_impaired: disp["visual_impaired"].as_i64().unwrap_or(0) == 1,
                        hearing_impaired: disp["hearing_impaired"].as_i64().unwrap_or(0) == 1,
                    }
                } else {
                    StreamDisposition {
                        default: false,
                        forced: false,
                        comment: false,
                        lyrics: false,
                        karaoke: false,
                        original: false,
                        dub: false,
                        visual_impaired: false,
                        hearing_impaired: false,
                    }
                };

                streams.push(StreamInfo {
                    index: index as u32,
                    codec_type,
                    codec_name,
                    language,
                    title,
                    disposition,
                });

                debug!(
                    "Stream {}: {} ({}) - Lang: {:?}, Title: {:?}",
                    index,
                    streams.last().unwrap().codec_type,
                    streams.last().unwrap().codec_name,
                    streams.last().unwrap().language,
                    streams.last().unwrap().title
                );
            }
        }

        Ok(streams)
    }

    async fn get_chapter_info<P: AsRef<Path>>(&self, input_path: P) -> Result<Vec<ChapterInfo>> {
        let input_path = input_path.as_ref();

        // Use the integrated FFmpeg wrapper
        debug!(
            "Using FFmpeg wrapper for chapter analysis: {}",
            input_path.display()
        );

        let output = match self
            .ffmpeg
            .run_ffprobe(&[
                "-v",
                "quiet",
                "-print_format",
                "json",
                "-show_chapters",
                &input_path.to_string_lossy(),
            ])
            .await
        {
            Ok(output) => output,
            Err(_) => {
                // Chapters are optional - don't fail if they don't exist
                debug!("No chapters found or chapter detection failed");
                return Ok(Vec::new());
            }
        };

        let json: Value = from_str(&output)?;

        let mut chapters = Vec::new();

        if let Some(chapter_array) = json["chapters"].as_array() {
            for chapter in chapter_array {
                let id = chapter["id"].as_u64().unwrap_or(0) as u32;
                let time_base = chapter["time_base"]
                    .as_str()
                    .unwrap_or("1/1000")
                    .to_string();
                let start = chapter["start"].as_u64().unwrap_or(0);
                let start_time = chapter["start_time"]
                    .as_str()
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(0.0);
                let end = chapter["end"].as_u64().unwrap_or(0);
                let end_time = chapter["end_time"]
                    .as_str()
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(0.0);
                let title = chapter["tags"]["title"].as_str().map(|s| s.to_string());

                chapters.push(ChapterInfo {
                    id,
                    time_base,
                    start,
                    start_time,
                    end,
                    end_time,
                    title: title.clone(),
                });

                debug!(
                    "Chapter {}: {:.2}s - {:.2}s - {:?}",
                    id, start_time, end_time, title
                );
            }
        }

        Ok(chapters)
    }

    async fn get_global_metadata<P: AsRef<Path>>(
        &self,
        input_path: P,
    ) -> Result<Vec<(String, String)>> {
        let input_path = input_path.as_ref();

        // Use the integrated FFmpeg wrapper
        debug!(
            "Using FFmpeg wrapper for metadata analysis: {}",
            input_path.display()
        );

        let output = match self
            .ffmpeg
            .run_ffprobe(&[
                "-v",
                "quiet",
                "-print_format",
                "json",
                "-show_format",
                &input_path.to_string_lossy(),
            ])
            .await
        {
            Ok(output) => output,
            Err(_) => {
                debug!("Failed to extract global metadata");
                return Ok(Vec::new());
            }
        };

        let json: Value = from_str(&output)?;

        let mut metadata = Vec::new();

        if let Some(format_obj) = json["format"].as_object() {
            if let Some(tags) = format_obj["tags"].as_object() {
                for (key, value) in tags {
                    if let Some(value_str) = value.as_str() {
                        metadata.push((key.clone(), value_str.to_string()));
                        debug!("Global metadata: {} = {}", key, value_str);
                    }
                }
            }
        }

        Ok(metadata)
    }

    fn build_mapping_arguments(&self, streams: &[StreamInfo]) -> Result<Vec<String>> {
        let mut args = Vec::new();

        // Simple 1:1 mapping: copy everything from input to output
        // Map video stream (first video stream only for encoding)
        // Note: When using filter_complex, this will be overridden to map [v] instead
        if streams.iter().any(|s| s.codec_type == "video") {
            args.push("-map".to_string());
            args.push("0:v:0".to_string()); // Use type-based mapping for first video stream
        }

        // Check if audio streams exist before mapping
        let has_audio = streams.iter().any(|s| s.codec_type == "audio");

        // Only map streams that actually exist
        if has_audio {
            args.push("-map".to_string());
            args.push("0:a".to_string()); // Copy all audio streams
        }

        args.extend(vec![
            "-map".to_string(),
            "0:s?".to_string(), // Copy all subtitle streams (optional)
            "-map".to_string(),
            "0:d?".to_string(), // Copy all data streams (optional)
            "-map".to_string(),
            "0:t?".to_string(), // Copy all attachment streams (optional)
        ]);

        // Set codecs for 1:1 copy (no transcoding except video)
        // Only set audio codec if we have audio streams
        if has_audio {
            args.extend(vec![
                "-c:a".to_string(),
                "copy".to_string(), // Copy audio streams as-is
            ]);
        }

        args.extend(vec![
            "-c:s".to_string(),
            "copy".to_string(), // Copy subtitle streams as-is
            "-c:d".to_string(),
            "copy".to_string(), // Copy data streams as-is
            "-c:t".to_string(),
            "copy".to_string(), // Copy attachment streams as-is
        ]);

        Ok(args)
    }

    fn build_filtered_mapping_arguments(
        &self,
        video_streams: &[StreamInfo],
        audio_streams: &[StreamInfo],
        subtitle_streams: &[StreamInfo],
        data_streams: &[StreamInfo],
    ) -> Result<Vec<String>> {
        let mut args = Vec::new();

        // Map video stream (first video stream only for encoding)
        if !video_streams.is_empty() {
            args.push("-map".to_string());
            args.push("0:v:0".to_string());
        }

        // Map filtered audio streams by their original indices
        for stream in audio_streams {
            args.push("-map".to_string());
            args.push(format!("0:{}", stream.index));
        }

        // Map filtered subtitle streams by their original indices
        for stream in subtitle_streams {
            args.push("-map".to_string());
            args.push(format!("0:{}", stream.index));
        }

        // Copy all data/attachment streams (these are usually small and important)
        if !data_streams.is_empty() {
            args.extend(vec![
                "-map".to_string(),
                "0:d?".to_string(), // Copy all data streams (optional)
                "-map".to_string(),
                "0:t?".to_string(), // Copy all attachment streams (optional)
            ]);
        }

        // Set codecs for stream copying
        if !audio_streams.is_empty() {
            args.extend(vec!["-c:a".to_string(), "copy".to_string()]);
        }

        if !subtitle_streams.is_empty() {
            args.extend(vec!["-c:s".to_string(), "copy".to_string()]);
        }

        if !data_streams.is_empty() {
            args.extend(vec![
                "-c:d".to_string(),
                "copy".to_string(),
                "-c:t".to_string(),
                "copy".to_string(),
            ]);
        }

        Ok(args)
    }

    fn filter_audio_streams(
        &self,
        streams: Vec<StreamInfo>,
        config: &AudioSelectionConfig,
    ) -> Result<Vec<StreamInfo>> {
        let original_count = streams.len();
        let mut filtered_streams = streams;

        // Filter by languages
        if let Some(languages) = &config.languages {
            filtered_streams.retain(|stream| {
                if let Some(lang) = &stream.language {
                    languages.iter().any(|pattern_lang| {
                        lang.to_lowercase().contains(&pattern_lang.to_lowercase())
                    })
                } else {
                    // If no language specified in stream, decide based on config
                    // For now, include streams with no language info
                    false
                }
            });
        }

        // Filter by codecs
        if let Some(codecs) = &config.codecs {
            filtered_streams.retain(|stream| {
                codecs.iter().any(|pattern_codec| {
                    stream
                        .codec_name
                        .to_lowercase()
                        .contains(&pattern_codec.to_lowercase())
                })
            });
        }

        // Filter by dispositions
        if let Some(dispositions) = &config.dispositions {
            filtered_streams.retain(|stream| {
                dispositions
                    .iter()
                    .any(|disposition| match disposition.to_lowercase().as_str() {
                        "default" => stream.disposition.default,
                        "forced" => stream.disposition.forced,
                        "original" => stream.disposition.original,
                        "dub" => stream.disposition.dub,
                        "comment" => stream.disposition.comment,
                        "lyrics" => stream.disposition.lyrics,
                        "karaoke" => stream.disposition.karaoke,
                        "visual_impaired" => stream.disposition.visual_impaired,
                        "hearing_impaired" => stream.disposition.hearing_impaired,
                        _ => false,
                    })
            });
        }

        // Filter by title patterns (regex)
        if let Some(title_patterns) = &config.title_patterns {
            filtered_streams.retain(|stream| {
                if let Some(title) = &stream.title {
                    title_patterns.iter().any(|pattern| {
                        match Regex::new(pattern) {
                            Ok(regex) => regex.is_match(title),
                            Err(_) => {
                                warn!(
                                    "Invalid regex pattern for audio title filtering: {}",
                                    pattern
                                );
                                // Fall back to simple substring matching
                                title.to_lowercase().contains(&pattern.to_lowercase())
                            }
                        }
                    })
                } else {
                    false
                }
            });
        }

        // Exclude commentary tracks
        if config.exclude_commentary {
            filtered_streams.retain(|stream| {
                !stream.disposition.comment
                    && stream.title.as_ref().is_none_or(|title| {
                        !title.to_lowercase().contains("commentary")
                            && !title.to_lowercase().contains("director")
                    })
            });
        }

        // Limit number of streams
        if let Some(max_streams) = config.max_streams {
            filtered_streams.truncate(max_streams);
        }

        debug!(
            "Audio streams filtered: {} -> {}",
            original_count,
            filtered_streams.len()
        );
        Ok(filtered_streams)
    }

    fn filter_subtitle_streams(
        &self,
        streams: Vec<StreamInfo>,
        config: &SubtitleSelectionConfig,
    ) -> Result<Vec<StreamInfo>> {
        let original_count = streams.len();
        let mut filtered_streams = streams;

        // Filter by languages
        if let Some(languages) = &config.languages {
            filtered_streams.retain(|stream| {
                if let Some(lang) = &stream.language {
                    languages.iter().any(|pattern_lang| {
                        lang.to_lowercase().contains(&pattern_lang.to_lowercase())
                    })
                } else {
                    false
                }
            });
        }

        // Filter by codecs
        if let Some(codecs) = &config.codecs {
            filtered_streams.retain(|stream| {
                codecs.iter().any(|pattern_codec| {
                    stream
                        .codec_name
                        .to_lowercase()
                        .contains(&pattern_codec.to_lowercase())
                })
            });
        }

        // Filter by dispositions
        if let Some(dispositions) = &config.dispositions {
            filtered_streams.retain(|stream| {
                dispositions
                    .iter()
                    .any(|disposition| match disposition.to_lowercase().as_str() {
                        "default" => stream.disposition.default,
                        "forced" => stream.disposition.forced,
                        "original" => stream.disposition.original,
                        "comment" => stream.disposition.comment,
                        "hearing_impaired" => stream.disposition.hearing_impaired,
                        "visual_impaired" => stream.disposition.visual_impaired,
                        _ => false,
                    })
            });
        }

        // Include forced subtitles only
        if config.include_forced_only {
            filtered_streams.retain(|stream| stream.disposition.forced);
        }

        // Filter by title patterns (regex)
        if let Some(title_patterns) = &config.title_patterns {
            filtered_streams.retain(|stream| {
                if let Some(title) = &stream.title {
                    title_patterns
                        .iter()
                        .any(|pattern| match Regex::new(pattern) {
                            Ok(regex) => regex.is_match(title),
                            Err(_) => {
                                warn!(
                                    "Invalid regex pattern for subtitle title filtering: {}",
                                    pattern
                                );
                                title.to_lowercase().contains(&pattern.to_lowercase())
                            }
                        })
                } else {
                    false
                }
            });
        }

        // Exclude commentary subtitles
        if config.exclude_commentary {
            filtered_streams.retain(|stream| {
                !stream.disposition.comment
                    && stream.title.as_ref().is_none_or(|title| {
                        !title.to_lowercase().contains("commentary")
                            && !title.to_lowercase().contains("director")
                    })
            });
        }

        // Limit number of streams
        if let Some(max_streams) = config.max_streams {
            filtered_streams.truncate(max_streams);
        }

        debug!(
            "Subtitle streams filtered: {} -> {}",
            original_count,
            filtered_streams.len()
        );
        Ok(filtered_streams)
    }

    pub fn get_metadata_args(
        &self,
        mapping: &StreamMapping,
        custom_title: Option<&str>,
    ) -> Vec<String> {
        let mut args = Vec::new();

        // Use bulk metadata and chapter mapping for better performance
        args.extend(vec!["-map_metadata".to_string(), "0".to_string()]);
        args.extend(vec!["-map_chapters".to_string(), "0".to_string()]);

        // Override title if provided
        if let Some(title) = custom_title {
            args.push("-metadata".to_string());
            args.push(format!("title={}", title));
        }

        // Note: Stream metadata and dispositions are preserved via -map_metadata 0
        // Only add explicit overrides if needed for specific dispositions

        // Preserve important dispositions that might not be transferred automatically
        for (audio_index, audio_stream) in mapping.audio_streams.iter().enumerate() {
            if audio_stream.disposition.default {
                args.push(format!("-disposition:a:{}", audio_index));
                args.push("default".to_string());
            }
        }

        args
    }

    pub fn validate_stream_preservation(&self, mapping: &StreamMapping) -> Result<()> {
        // Ensure we have at least one video stream
        if mapping.video_streams.is_empty() {
            return Err(Error::analysis("No video streams found in input"));
        }

        info!("Stream preservation validation:");
        info!("  Video streams: {}", mapping.video_streams.len());
        info!("  Audio streams: {}", mapping.audio_streams.len());
        info!("  Subtitle streams: {}", mapping.subtitle_streams.len());
        info!("  Data streams: {}", mapping.data_streams.len());
        info!("  Chapters: {}", mapping.chapters.len());
        info!("  Global metadata entries: {}", mapping.metadata.len());

        // Warn about any potential issues
        if mapping.audio_streams.is_empty() {
            warn!("No audio streams found - output will be video-only");
        }

        if mapping.subtitle_streams.is_empty() {
            debug!("No subtitle streams found");
        } else {
            info!(
                "Preserving {} subtitle streams",
                mapping.subtitle_streams.len()
            );
        }

        if mapping.chapters.is_empty() {
            debug!("No chapters found");
        } else {
            info!("Preserving {} chapters", mapping.chapters.len());
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::FfmpegWrapper;

    #[tokio::test]
    async fn test_stream_preservation_creation() {
        let ffmpeg = FfmpegWrapper::new("ffmpeg".to_string(), "ffprobe".to_string());
        let _preservation = StreamPreservation::new(ffmpeg);

        // Test basic functionality
        // Placeholder test - would need actual test files to verify functionality
    }

    #[test]
    fn test_mapping_arguments_construction() {
        let ffmpeg = FfmpegWrapper::new("ffmpeg".to_string(), "ffprobe".to_string());
        let preservation = StreamPreservation::new(ffmpeg);

        let streams = vec![
            // Sample video stream
            StreamInfo {
                index: 0,
                codec_type: "video".to_string(),
                codec_name: "h264".to_string(),
                language: None,
                title: None,
                disposition: StreamDisposition {
                    default: true,
                    forced: false,
                    comment: false,
                    lyrics: false,
                    karaoke: false,
                    original: false,
                    dub: false,
                    visual_impaired: false,
                    hearing_impaired: false,
                },
            },
            // Sample audio stream
            StreamInfo {
                index: 1,
                codec_type: "audio".to_string(),
                codec_name: "aac".to_string(),
                language: Some("eng".to_string()),
                title: Some("English Audio".to_string()),
                disposition: StreamDisposition {
                    default: true,
                    forced: false,
                    comment: false,
                    lyrics: false,
                    karaoke: false,
                    original: true,
                    dub: false,
                    visual_impaired: false,
                    hearing_impaired: false,
                },
            },
        ];

        let mapping_args = preservation.build_mapping_arguments(&streams).unwrap();

        assert!(mapping_args.contains(&"-map".to_string()));
        assert!(mapping_args.contains(&"0:v:0".to_string())); // Type-based video mapping
        assert!(mapping_args.contains(&"0:a".to_string())); // Bulk audio mapping
        assert!(mapping_args.contains(&"0:s?".to_string())); // Optional subtitle mapping
        assert!(mapping_args.contains(&"-c:a".to_string()));
        assert!(mapping_args.contains(&"copy".to_string()));
    }

    #[test]
    fn test_mapping_arguments_video_only() {
        let ffmpeg = FfmpegWrapper::new("ffmpeg".to_string(), "ffprobe".to_string());
        let preservation = StreamPreservation::new(ffmpeg);

        let streams = vec![
            // Sample video stream only
            StreamInfo {
                index: 0,
                codec_type: "video".to_string(),
                codec_name: "h264".to_string(),
                language: None,
                title: None,
                disposition: StreamDisposition {
                    default: true,
                    forced: false,
                    comment: false,
                    lyrics: false,
                    karaoke: false,
                    original: false,
                    dub: false,
                    visual_impaired: false,
                    hearing_impaired: false,
                },
            },
        ];

        let mapping_args = preservation.build_mapping_arguments(&streams).unwrap();

        assert!(mapping_args.contains(&"-map".to_string()));
        assert!(mapping_args.contains(&"0:v:0".to_string())); // Type-based video mapping
        assert!(!mapping_args.contains(&"0:a".to_string())); // Should not map audio streams
        assert!(mapping_args.contains(&"0:s?".to_string())); // Optional subtitle mapping
        assert!(!mapping_args.contains(&"-c:a".to_string())); // Should not set audio codec
    }

    #[test]
    fn test_audio_stream_filtering_by_language() {
        let ffmpeg = FfmpegWrapper::new("ffmpeg".to_string(), "ffprobe".to_string());
        let preservation = StreamPreservation::new(ffmpeg);

        let streams = vec![
            StreamInfo {
                index: 1,
                codec_type: "audio".to_string(),
                codec_name: "aac".to_string(),
                language: Some("eng".to_string()),
                title: Some("English Audio".to_string()),
                disposition: StreamDisposition {
                    default: true,
                    forced: false,
                    comment: false,
                    lyrics: false,
                    karaoke: false,
                    original: true,
                    dub: false,
                    visual_impaired: false,
                    hearing_impaired: false,
                },
            },
            StreamInfo {
                index: 2,
                codec_type: "audio".to_string(),
                codec_name: "aac".to_string(),
                language: Some("jpn".to_string()),
                title: Some("Japanese Audio".to_string()),
                disposition: StreamDisposition {
                    default: false,
                    forced: false,
                    comment: false,
                    lyrics: false,
                    karaoke: false,
                    original: true,
                    dub: false,
                    visual_impaired: false,
                    hearing_impaired: false,
                },
            },
            StreamInfo {
                index: 3,
                codec_type: "audio".to_string(),
                codec_name: "aac".to_string(),
                language: Some("ger".to_string()),
                title: Some("German Audio".to_string()),
                disposition: StreamDisposition {
                    default: false,
                    forced: false,
                    comment: false,
                    lyrics: false,
                    karaoke: false,
                    original: false,
                    dub: true,
                    visual_impaired: false,
                    hearing_impaired: false,
                },
            },
        ];

        let config = AudioSelectionConfig {
            languages: Some(vec!["eng".to_string(), "jpn".to_string()]),
            ..Default::default()
        };

        let filtered = preservation.filter_audio_streams(streams, &config).unwrap();

        assert_eq!(filtered.len(), 2);
        assert_eq!(filtered[0].language.as_ref().unwrap(), "eng");
        assert_eq!(filtered[1].language.as_ref().unwrap(), "jpn");
    }

    #[test]
    fn test_audio_stream_filtering_exclude_commentary() {
        let ffmpeg = FfmpegWrapper::new("ffmpeg".to_string(), "ffprobe".to_string());
        let preservation = StreamPreservation::new(ffmpeg);

        let streams = vec![
            StreamInfo {
                index: 1,
                codec_type: "audio".to_string(),
                codec_name: "aac".to_string(),
                language: Some("eng".to_string()),
                title: Some("English Audio".to_string()),
                disposition: StreamDisposition {
                    default: true,
                    forced: false,
                    comment: false,
                    lyrics: false,
                    karaoke: false,
                    original: true,
                    dub: false,
                    visual_impaired: false,
                    hearing_impaired: false,
                },
            },
            StreamInfo {
                index: 2,
                codec_type: "audio".to_string(),
                codec_name: "aac".to_string(),
                language: Some("eng".to_string()),
                title: Some("Director Commentary".to_string()),
                disposition: StreamDisposition {
                    default: false,
                    forced: false,
                    comment: true,
                    lyrics: false,
                    karaoke: false,
                    original: false,
                    dub: false,
                    visual_impaired: false,
                    hearing_impaired: false,
                },
            },
        ];

        let config = AudioSelectionConfig {
            exclude_commentary: true,
            ..Default::default()
        };

        let filtered = preservation.filter_audio_streams(streams, &config).unwrap();

        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].title.as_ref().unwrap(), "English Audio");
    }

    #[test]
    fn test_subtitle_stream_filtering_forced_only() {
        let ffmpeg = FfmpegWrapper::new("ffmpeg".to_string(), "ffprobe".to_string());
        let preservation = StreamPreservation::new(ffmpeg);

        let streams = vec![
            StreamInfo {
                index: 4,
                codec_type: "subtitle".to_string(),
                codec_name: "subrip".to_string(),
                language: Some("eng".to_string()),
                title: Some("English Subtitles".to_string()),
                disposition: StreamDisposition {
                    default: true,
                    forced: false,
                    comment: false,
                    lyrics: false,
                    karaoke: false,
                    original: false,
                    dub: false,
                    visual_impaired: false,
                    hearing_impaired: false,
                },
            },
            StreamInfo {
                index: 5,
                codec_type: "subtitle".to_string(),
                codec_name: "subrip".to_string(),
                language: Some("eng".to_string()),
                title: Some("English Forced".to_string()),
                disposition: StreamDisposition {
                    default: false,
                    forced: true,
                    comment: false,
                    lyrics: false,
                    karaoke: false,
                    original: false,
                    dub: false,
                    visual_impaired: false,
                    hearing_impaired: false,
                },
            },
        ];

        let config = SubtitleSelectionConfig {
            include_forced_only: true,
            ..Default::default()
        };

        let filtered = preservation
            .filter_subtitle_streams(streams, &config)
            .unwrap();

        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].title.as_ref().unwrap(), "English Forced");
        assert!(filtered[0].disposition.forced);
    }
}
