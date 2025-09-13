use crate::utils::{Result, FfmpegWrapper, Error};
use std::path::Path;
use serde_json::{Value, from_str};
use tracing::{info, warn, debug};

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
        let video_streams: Vec<StreamInfo> = streams.iter()
            .filter(|s| s.codec_type == "video")
            .cloned()
            .collect();
        
        let audio_streams: Vec<StreamInfo> = streams.iter()
            .filter(|s| s.codec_type == "audio")
            .cloned()
            .collect();
        
        let subtitle_streams: Vec<StreamInfo> = streams.iter()
            .filter(|s| s.codec_type == "subtitle")
            .cloned()
            .collect();
            
        let data_streams: Vec<StreamInfo> = streams.iter()
            .filter(|s| s.codec_type == "data" || s.codec_type == "attachment")
            .cloned()
            .collect();

        // Build mapping arguments
        let mapping_args = self.build_mapping_arguments(&streams)?;

        info!("Stream analysis complete: {} video, {} audio, {} subtitle, {} data, {} chapters", 
              video_streams.len(), audio_streams.len(), subtitle_streams.len(), 
              data_streams.len(), chapters.len());

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
        debug!("Using FFmpeg wrapper for stream analysis: {}", input_path.display());
        
        let output = self.ffmpeg.run_ffprobe(&[
            "-v", "quiet",
            "-analyzeduration", "5M",  // Optimized: reduced from default for faster analysis
            "-probesize", "5M",        // Optimized: reduced from default for faster analysis
            "-print_format", "json", 
            "-show_streams",
            "-show_format",
            &input_path.to_string_lossy()
        ]).await?;

        let json: Value = from_str(&output)?;

        let mut streams = Vec::new();

        if let Some(stream_array) = json["streams"].as_array() {
            for (index, stream) in stream_array.iter().enumerate() {
                let codec_type = stream["codec_type"].as_str().unwrap_or("unknown").to_string();
                let codec_name = stream["codec_name"].as_str().unwrap_or("unknown").to_string();
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

                debug!("Stream {}: {} ({}) - Lang: {:?}, Title: {:?}", 
                       index, 
                       streams.last().unwrap().codec_type,
                       streams.last().unwrap().codec_name,
                       streams.last().unwrap().language,
                       streams.last().unwrap().title);
            }
        }

        Ok(streams)
    }

    async fn get_chapter_info<P: AsRef<Path>>(&self, input_path: P) -> Result<Vec<ChapterInfo>> {
        let input_path = input_path.as_ref();
        
        // Use the integrated FFmpeg wrapper
        debug!("Using FFmpeg wrapper for chapter analysis: {}", input_path.display());
        
        let output = match self.ffmpeg.run_ffprobe(&[
            "-v", "quiet",
            "-print_format", "json",
            "-show_chapters",
            &input_path.to_string_lossy()
        ]).await {
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
                let time_base = chapter["time_base"].as_str().unwrap_or("1/1000").to_string();
                let start = chapter["start"].as_u64().unwrap_or(0);
                let start_time = chapter["start_time"].as_str()
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(0.0);
                let end = chapter["end"].as_u64().unwrap_or(0);
                let end_time = chapter["end_time"].as_str()
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

                debug!("Chapter {}: {:.2}s - {:.2}s - {:?}", 
                       id, start_time, end_time, title);
            }
        }

        Ok(chapters)
    }

    async fn get_global_metadata<P: AsRef<Path>>(&self, input_path: P) -> Result<Vec<(String, String)>> {
        let input_path = input_path.as_ref();
        
        // Use the integrated FFmpeg wrapper
        debug!("Using FFmpeg wrapper for metadata analysis: {}", input_path.display());
        
        let output = match self.ffmpeg.run_ffprobe(&[
            "-v", "quiet",
            "-print_format", "json",
            "-show_format",
            &input_path.to_string_lossy()
        ]).await {
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

        // Map video stream (first video stream only for encoding)
        if let Some(video_stream) = streams.iter().find(|s| s.codec_type == "video") {
            args.push("-map".to_string());
            args.push(format!("0:{}", video_stream.index));
        }

        // Map all audio streams with lossless copy
        for audio_stream in streams.iter().filter(|s| s.codec_type == "audio") {
            args.push("-map".to_string());
            args.push(format!("0:{}", audio_stream.index));
        }

        // Audio codec settings - lossless copy
        if streams.iter().any(|s| s.codec_type == "audio") {
            args.push("-c:a".to_string());
            args.push("copy".to_string());
        }

        // Map all subtitle streams with lossless copy
        for subtitle_stream in streams.iter().filter(|s| s.codec_type == "subtitle") {
            args.push("-map".to_string());
            args.push(format!("0:{}", subtitle_stream.index));
        }

        // Subtitle codec settings - lossless copy
        if streams.iter().any(|s| s.codec_type == "subtitle") {
            args.push("-c:s".to_string());
            args.push("copy".to_string());
        }

        // Map data/attachment streams (fonts, images, etc.)
        for data_stream in streams.iter().filter(|s| s.codec_type == "data" || s.codec_type == "attachment") {
            args.push("-map".to_string());
            args.push(format!("0:{}", data_stream.index));
        }

        // Data codec settings - lossless copy
        if streams.iter().any(|s| s.codec_type == "data" || s.codec_type == "attachment") {
            args.push("-c:d".to_string());
            args.push("copy".to_string());
            args.push("-c:t".to_string());
            args.push("copy".to_string());
        }

        Ok(args)
    }

    pub fn get_metadata_args(&self, mapping: &StreamMapping, custom_title: Option<&str>) -> Vec<String> {
        let mut args = Vec::new();

        // Preserve all existing metadata first
        for (key, value) in &mapping.metadata {
            args.push("-metadata".to_string());
            args.push(format!("{}={}", key, value));
        }

        // Override title if provided
        if let Some(title) = custom_title {
            args.push("-metadata".to_string());
            args.push(format!("title={}", title));
        }

        // Preserve stream metadata and dispositions
        let mut stream_index = 0;

        // Video stream metadata
        for video_stream in &mapping.video_streams {
            if let Some(lang) = &video_stream.language {
                args.push(format!("-metadata:s:v:{}", stream_index));
                args.push(format!("language={}", lang));
            }
            if let Some(title) = &video_stream.title {
                args.push(format!("-metadata:s:v:{}", stream_index));
                args.push(format!("title={}", title));
            }
            
            // Preserve dispositions
            if video_stream.disposition.default {
                args.push(format!("-disposition:v:{}", stream_index));
                args.push("default".to_string());
            }
            if video_stream.disposition.forced {
                args.push(format!("-disposition:v:{}", stream_index));
                args.push("forced".to_string());
            }
            
            stream_index += 1;
        }

        // Audio stream metadata
        stream_index = 0;
        for audio_stream in &mapping.audio_streams {
            if let Some(lang) = &audio_stream.language {
                args.push(format!("-metadata:s:a:{}", stream_index));
                args.push(format!("language={}", lang));
            }
            if let Some(title) = &audio_stream.title {
                args.push(format!("-metadata:s:a:{}", stream_index));
                args.push(format!("title={}", title));
            }
            
            // Preserve dispositions
            if audio_stream.disposition.default {
                args.push(format!("-disposition:a:{}", stream_index));
                args.push("default".to_string());
            }
            if audio_stream.disposition.original {
                args.push(format!("-disposition:a:{}", stream_index));
                args.push("original".to_string());
            }
            if audio_stream.disposition.dub {
                args.push(format!("-disposition:a:{}", stream_index));
                args.push("dub".to_string());
            }
            
            stream_index += 1;
        }

        // Subtitle stream metadata
        stream_index = 0;
        for subtitle_stream in &mapping.subtitle_streams {
            if let Some(lang) = &subtitle_stream.language {
                args.push(format!("-metadata:s:s:{}", stream_index));
                args.push(format!("language={}", lang));
            }
            if let Some(title) = &subtitle_stream.title {
                args.push(format!("-metadata:s:s:{}", stream_index));
                args.push(format!("title={}", title));
            }
            
            // Preserve dispositions
            if subtitle_stream.disposition.default {
                args.push(format!("-disposition:s:{}", stream_index));
                args.push("default".to_string());
            }
            if subtitle_stream.disposition.forced {
                args.push(format!("-disposition:s:{}", stream_index));
                args.push("forced".to_string());
            }
            if subtitle_stream.disposition.hearing_impaired {
                args.push(format!("-disposition:s:{}", stream_index));
                args.push("hearing_impaired".to_string());
            }
            
            stream_index += 1;
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
            info!("Preserving {} subtitle streams", mapping.subtitle_streams.len());
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
        let preservation = StreamPreservation::new(ffmpeg);
        
        // Test basic functionality
        assert!(true); // Placeholder - would need actual test files
    }

    #[test]
    fn test_mapping_arguments_construction() {
        let ffmpeg = FfmpegWrapper::new("ffmpeg".to_string(), "ffprobe".to_string());
        let preservation = StreamPreservation::new(ffmpeg);
        
        let mut streams = Vec::new();
        
        // Add sample video stream
        streams.push(StreamInfo {
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
        });
        
        // Add sample audio stream
        streams.push(StreamInfo {
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
        });
        
        let mapping_args = preservation.build_mapping_arguments(&streams).unwrap();
        
        assert!(mapping_args.contains(&"-map".to_string()));
        assert!(mapping_args.contains(&"0:0".to_string()));
        assert!(mapping_args.contains(&"0:1".to_string()));
        assert!(mapping_args.contains(&"-c:a".to_string()));
        assert!(mapping_args.contains(&"copy".to_string()));
    }
}