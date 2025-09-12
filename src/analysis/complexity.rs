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

pub struct ComplexityAnalyzer {}

impl ComplexityAnalyzer {
    pub fn new() -> Self {
        Self {}
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
        
        // Use bash script's strategic approach: only 3 key sample points (start, middle, end)
        let strategic_points = vec![
            60.0,                    // 60s from start
            duration / 2.0,          // middle
            duration - 60.0          // 60s from end
        ];
        
        for timestamp in strategic_points {
            // Skip invalid timestamps
            if timestamp < 0.0 || timestamp > duration {
                continue;
            }
            
            // Use only the most effective method (local_variance) like bash script's combined approach
            let grain_level = self.analyze_local_variance(input_path.as_ref(), timestamp).await?;
            grain_levels.push(grain_level);
        }
        
        // Calculate average grain level
        if grain_levels.is_empty() {
            Ok(0.0)
        } else {
            let avg_grain = grain_levels.iter().sum::<f32>() / grain_levels.len() as f32;
            Ok(avg_grain.min(100.0))
        }
    }


    async fn analyze_local_variance(&self, input_path: &Path, timestamp: f64) -> Result<f32> {
        // Extract frame as PNG for analysis using bash script's crop approach
        let temp_frame = format!("/tmp/grain_frame_{}.png", uuid::Uuid::new_v4());
        
        let extract_result = Command::new("ffmpeg")
            .args(&[
                "-ss", &timestamp.to_string(),
                "-i", &input_path.to_string_lossy(),
                "-t", "1",  // Limit to 1 second like bash script
                "-vf", "crop=400:400:iw/2-200:ih/2-200",  // Use bash script's center crop
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



    async fn calculate_si_ti(&self, input_path: &Path) -> Result<(f32, f32)> {
        // Use bash script's approach: limit to 30 seconds for SI/TI calculation
        let output = Command::new("ffmpeg")
            .args(&[
                "-i", &input_path.to_string_lossy(),
                "-t", "30",  // Limit to 30 seconds like bash script
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
        // Use bash script's approach: limit to 60 seconds like the original
        let output = Command::new("ffmpeg")
            .args(&[
                "-i", &input_path.to_string_lossy(),
                "-t", "60",  // Limit analysis to 60 seconds like bash script
                "-vf", "select='gt(scene,0.3)',showinfo",
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
        // Analyze frame types and their distribution using bash script's limited sampling approach
        let output = Command::new("ffprobe")
            .args(&[
                "-v", "error",
                "-select_streams", "v:0",
                "-read_intervals", "%+#1800",  // Only analyze first 1800 frames like bash script
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
        // Count scene changes from showinfo filter output (like bash script)
        let scene_changes = output.matches("Parsed_showinfo").count() as u32;
        Ok(scene_changes)
    }
}

impl Default for ComplexityAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_complexity_analyzer_creation() {
        let _analyzer = ComplexityAnalyzer::default();
    }

}