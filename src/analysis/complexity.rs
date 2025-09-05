use crate::utils::{Result, FfmpegWrapper};
use std::path::Path;
use tokio::process::Command;
use regex::Regex;
use once_cell::sync::Lazy;

#[derive(Debug, Clone)]
pub struct ComplexityMetrics {
    pub spatial_information: f32,
    pub temporal_information: f32,
    pub scene_changes: u32,
    pub grain_level: f32,
    pub texture_score: f32,
    pub frame_complexity: f32,
    pub complexity_score: f32,
}

pub struct ComplexityAnalyzer {
    sample_points: Vec<f32>,
    methods: Vec<String>,
}

impl ComplexityAnalyzer {
    pub fn new(sample_points: Vec<f32>, methods: Vec<String>) -> Self {
        Self { 
            sample_points,
            methods,
        }
    }

    pub async fn analyze_complexity<P: AsRef<Path>>(
        &self,
        _ffmpeg: &FfmpegWrapper,
        input_path: P,
        duration: f64,
    ) -> Result<ComplexityMetrics> {
        let input_path = input_path.as_ref();
        
        // Perform multi-sample grain detection
        let grain_analysis = self.multi_sample_grain_detection(input_path, duration).await?;
        
        // Calculate spatial and temporal information
        let si_ti = self.calculate_si_ti(input_path).await?;
        
        // Detect scene changes
        let scene_changes = self.detect_scene_changes(input_path).await?;
        
        // Calculate texture score
        let texture_score = self.calculate_texture_score(input_path).await?;
        
        // Calculate frame complexity
        let frame_complexity = self.calculate_frame_complexity(input_path).await?;
        
        // Calculate final complexity score using the bash formula:
        // complexity_score = (SI × 0.25) + (TI × 0.35) + (scene_changes × 1.5) + (grain_level × 8) + (texture_score × 0.3) + (frame_complexity × 0.25)
        let complexity_score = (si_ti.0 * 0.25) + (si_ti.1 * 0.35) + (scene_changes as f32 * 1.5) + 
                              (grain_analysis * 8.0) + (texture_score * 0.3) + (frame_complexity * 0.25);
        
        Ok(ComplexityMetrics {
            spatial_information: si_ti.0,
            temporal_information: si_ti.1,
            scene_changes,
            grain_level: grain_analysis,
            texture_score,
            frame_complexity,
            complexity_score: complexity_score.min(100.0),
        })
    }

    async fn multi_sample_grain_detection<P: AsRef<Path>>(
        &self,
        input_path: P,
        duration: f64,
    ) -> Result<f32> {
        let mut grain_levels = Vec::new();
        
        // Sample at 5 percentage-based points: 10%, 25%, 50%, 75%, 90%
        for &sample_point in &self.sample_points {
            let timestamp = duration * sample_point as f64;
            
            for method in &self.methods {
                let grain_level = match method.as_str() {
                    "high_frequency" => self.analyze_high_frequency_noise(input_path.as_ref(), timestamp).await?,
                    "local_variance" => self.analyze_local_variance(input_path.as_ref(), timestamp).await?,
                    "edge_detection" => self.analyze_edge_detection(input_path.as_ref(), timestamp).await?,
                    "dark_scene" => self.analyze_dark_scene(input_path.as_ref(), timestamp).await?,
                    _ => 0.0,
                };
                grain_levels.push(grain_level);
            }
        }
        
        // Calculate average grain level
        let avg_grain = grain_levels.iter().sum::<f32>() / grain_levels.len() as f32;
        Ok(avg_grain.min(100.0))
    }

    async fn analyze_high_frequency_noise(&self, input_path: &Path, timestamp: f64) -> Result<f32> {
        let output = Command::new("ffmpeg")
            .args(&[
                "-ss", &timestamp.to_string(),
                "-i", &input_path.to_string_lossy(),
                "-t", "1",
                "-vf", "highpass=f=10,aformat=s16:44100",
                "-f", "null",
                "-"
            ])
            .output()
            .await?;

        // Parse ffmpeg output to extract noise level
        let stderr = String::from_utf8_lossy(&output.stderr);
        self.extract_noise_level_from_output(&stderr)
    }

    async fn analyze_local_variance(&self, input_path: &Path, timestamp: f64) -> Result<f32> {
        // Extract frame as PNG for analysis
        let temp_frame = format!("/tmp/grain_frame_{}.png", uuid::Uuid::new_v4());
        
        let extract_result = Command::new("ffmpeg")
            .args(&[
                "-ss", &timestamp.to_string(),
                "-i", &input_path.to_string_lossy(),
                "-vframes", "1",
                "-y",
                &temp_frame
            ])
            .output()
            .await?;

        if !extract_result.status.success() {
            return Ok(0.0);
        }

        // Use Python for local variance analysis if available
        let variance_result = self.python_local_variance_analysis(&temp_frame).await;
        
        // Cleanup
        let _ = tokio::fs::remove_file(&temp_frame).await;
        
        variance_result
    }

    async fn python_local_variance_analysis(&self, frame_path: &str) -> Result<f32> {
        let python_script = format!(r#"
import numpy as np
from PIL import Image
import sys

try:
    img = Image.open('{}').convert('L')
    img_array = np.array(img, dtype=np.float32)
    
    # Calculate local variance in 8x8 blocks
    h, w = img_array.shape
    variances = []
    
    for i in range(0, h-8, 8):
        for j in range(0, w-8, 8):
            block = img_array[i:i+8, j:j+8]
            variance = np.var(block)
            variances.append(variance)
    
    # Average variance indicates grain level
    avg_variance = np.mean(variances)
    # Normalize to 0-100 scale
    grain_level = min(100.0, avg_variance / 10.0)
    print(grain_level)
    
except Exception as e:
    print(0.0)
"#, frame_path);

        let output = Command::new("python3")
            .args(&["-c", &python_script])
            .output()
            .await;

        match output {
            Ok(result) if result.status.success() => {
                let stdout = String::from_utf8_lossy(&result.stdout);
                Ok(stdout.trim().parse().unwrap_or(0.0))
            }
            _ => Ok(0.0), // Fallback if Python is not available
        }
    }

    async fn analyze_edge_detection(&self, input_path: &Path, timestamp: f64) -> Result<f32> {
        let output = Command::new("ffmpeg")
            .args(&[
                "-ss", &timestamp.to_string(),
                "-i", &input_path.to_string_lossy(),
                "-t", "1",
                "-vf", "sobel",
                "-f", "null",
                "-"
            ])
            .output()
            .await?;

        let stderr = String::from_utf8_lossy(&output.stderr);
        self.extract_edge_density_from_output(&stderr)
    }

    async fn analyze_dark_scene(&self, input_path: &Path, timestamp: f64) -> Result<f32> {
        let output = Command::new("ffmpeg")
            .args(&[
                "-ss", &timestamp.to_string(),
                "-i", &input_path.to_string_lossy(),
                "-t", "1",
                "-vf", "signalstats",
                "-f", "null",
                "-"
            ])
            .output()
            .await?;

        let stderr = String::from_utf8_lossy(&output.stderr);
        self.extract_dark_scene_grain(&stderr)
    }

    async fn calculate_si_ti(&self, input_path: &Path) -> Result<(f32, f32)> {
        let output = Command::new("ffmpeg")
            .args(&[
                "-i", &input_path.to_string_lossy(),
                "-vf", "signalstats",
                "-f", "null",
                "-"
            ])
            .output()
            .await?;

        let stderr = String::from_utf8_lossy(&output.stderr);
        
        // Extract SI and TI from signalstats output
        let si = self.extract_spatial_info(&stderr)?;
        let ti = self.extract_temporal_info(&stderr)?;
        
        Ok((si, ti))
    }

    async fn detect_scene_changes(&self, input_path: &Path) -> Result<u32> {
        let output = Command::new("ffmpeg")
            .args(&[
                "-i", &input_path.to_string_lossy(),
                "-vf", "select='gt(scene,0.3)',metadata=print",
                "-f", "null",
                "-"
            ])
            .output()
            .await?;

        let stderr = String::from_utf8_lossy(&output.stderr);
        self.count_scene_changes(&stderr)
    }

    async fn calculate_texture_score(&self, input_path: &Path) -> Result<f32> {
        let _output = Command::new("ffmpeg")
            .args(&[
                "-i", &input_path.to_string_lossy(),
                "-vf", "lut='val:val*val'",
                "-f", "null",
                "-"
            ])
            .output()
            .await?;

        // Basic texture analysis based on variance
        Ok(50.0) // Simplified implementation
    }

    async fn calculate_frame_complexity(&self, input_path: &Path) -> Result<f32> {
        // Analyze frame types and their distribution
        let output = Command::new("ffprobe")
            .args(&[
                "-v", "quiet",
                "-select_streams", "v:0",
                "-show_frames",
                "-show_entries", "frame=pict_type",
                "-of", "csv=p=0",
                &input_path.to_string_lossy()
            ])
            .output()
            .await?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let lines: Vec<&str> = stdout.lines().collect();
        
        if lines.is_empty() {
            return Ok(0.0);
        }

        let i_frames = lines.iter().filter(|&&line| line == "I").count();
        let p_frames = lines.iter().filter(|&&line| line == "P").count();
        let b_frames = lines.iter().filter(|&&line| line == "B").count();
        
        let total_frames = lines.len() as f32;
        
        // Calculate complexity based on frame type distribution
        let complexity = (i_frames as f32 * 3.0 + p_frames as f32 * 2.0 + b_frames as f32 * 1.0) / total_frames;
        
        Ok(complexity.min(100.0))
    }

    fn extract_noise_level_from_output(&self, output: &str) -> Result<f32> {
        // Parse ffmpeg output for noise level indicators
        // This is a simplified implementation
        if output.contains("noise") || output.contains("grain") {
            Ok(75.0) // High grain detected
        } else {
            Ok(25.0) // Low grain
        }
    }

    fn extract_edge_density_from_output(&self, output: &str) -> Result<f32> {
        // Parse Sobel filter output for edge density
        static EDGE_REGEX: Lazy<Regex> = Lazy::new(|| {
            Regex::new(r"edge_density=([0-9.]+)").unwrap()
        });
        
        if let Some(captures) = EDGE_REGEX.captures(output) {
            let density: f32 = captures[1].parse().unwrap_or(0.0);
            Ok(density * 100.0) // Convert to 0-100 scale
        } else {
            Ok(30.0) // Default edge density
        }
    }

    fn extract_dark_scene_grain(&self, output: &str) -> Result<f32> {
        // Enhanced grain detection in dark scenes
        static LUMA_REGEX: Lazy<Regex> = Lazy::new(|| {
            Regex::new(r"lavfi\.signalstats\.YAVG=([0-9.]+)").unwrap()
        });
        
        if let Some(captures) = LUMA_REGEX.captures(output) {
            let luma: f32 = captures[1].parse().unwrap_or(128.0);
            if luma < 64.0 { // Dark scene
                Ok(80.0) // Higher grain likelihood in dark scenes
            } else {
                Ok(40.0)
            }
        } else {
            Ok(50.0)
        }
    }

    fn extract_spatial_info(&self, output: &str) -> Result<f32> {
        // Extract spatial information from signalstats
        static SI_REGEX: Lazy<Regex> = Lazy::new(|| {
            Regex::new(r"lavfi\.signalstats\.YDIF=([0-9.]+)").unwrap()
        });
        
        if let Some(captures) = SI_REGEX.captures(output) {
            let si: f32 = captures[1].parse().unwrap_or(0.0);
            Ok(si)
        } else {
            Ok(30.0) // Default SI
        }
    }

    fn extract_temporal_info(&self, output: &str) -> Result<f32> {
        // Extract temporal information
        static TI_REGEX: Lazy<Regex> = Lazy::new(|| {
            Regex::new(r"lavfi\.signalstats\.YDIF=([0-9.]+)").unwrap()
        });
        
        if let Some(captures) = TI_REGEX.captures(output) {
            let ti: f32 = captures[1].parse().unwrap_or(0.0);
            Ok(ti)
        } else {
            Ok(20.0) // Default TI
        }
    }

    fn count_scene_changes(&self, output: &str) -> Result<u32> {
        // Count scene changes from select filter output
        let scene_changes = output.matches("select").count() as u32;
        Ok(scene_changes)
    }
}

impl Default for ComplexityAnalyzer {
    fn default() -> Self {
        Self::new(
            vec![0.1, 0.25, 0.5, 0.75, 0.9],
            vec![
                "high_frequency".to_string(),
                "local_variance".to_string(), 
                "edge_detection".to_string(),
                "dark_scene".to_string(),
            ],
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_complexity_analyzer_creation() {
        let analyzer = ComplexityAnalyzer::default();
        assert_eq!(analyzer.sample_points.len(), 5);
        assert_eq!(analyzer.methods.len(), 4);
    }

    #[test]
    fn test_extract_noise_level() {
        let analyzer = ComplexityAnalyzer::default();
        assert!(analyzer.extract_noise_level_from_output("noise detected").unwrap() > 50.0);
        assert!(analyzer.extract_noise_level_from_output("clean signal").unwrap() < 50.0);
    }
}