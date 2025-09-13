use crate::utils::{Error, Result};
use std::path::{Path, PathBuf};
use uuid::Uuid;
use walkdir::WalkDir;

const VIDEO_EXTENSIONS: &[&str] = &[".mkv", ".mp4", ".mov", ".m4v", ".avi", ".webm"];

pub fn find_video_files<P: AsRef<Path>>(path: P) -> Result<Vec<PathBuf>> {
    let path = path.as_ref();

    if !path.exists() {
        return Err(Error::validation(format!(
            "Path does not exist: {}",
            path.display()
        )));
    }

    let mut video_files = Vec::new();

    if path.is_file() {
        if is_video_file(path) {
            video_files.push(path.to_path_buf());
        } else {
            return Err(Error::validation(format!(
                "File is not a supported video format: {}",
                path.display()
            )));
        }
    } else if path.is_dir() {
        for entry in WalkDir::new(path)
            .follow_links(false)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let path = entry.path();
            if path.is_file() && is_video_file(path) {
                video_files.push(path.to_path_buf());
            }
        }

        if video_files.is_empty() {
            return Err(Error::validation(format!(
                "No supported video files found in directory: {}",
                path.display()
            )));
        }

        video_files.sort();
    }

    Ok(video_files)
}

pub fn is_video_file<P: AsRef<Path>>(path: P) -> bool {
    let path = path.as_ref();

    if let Some(extension) = path.extension() {
        if let Some(ext_str) = extension.to_str() {
            let ext_lower = format!(".{}", ext_str.to_lowercase());
            return VIDEO_EXTENSIONS.contains(&ext_lower.as_str());
        }
    }

    false
}

pub fn generate_uuid_filename<P: AsRef<Path>, Q: AsRef<Path>>(
    input_path: P,
    output_dir: Option<Q>,
) -> PathBuf {
    let input_path = input_path.as_ref();
    let uuid = Uuid::new_v4();

    let file_stem = input_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("output");

    let extension = input_path
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("mkv");

    let filename = format!("{}_{}.{}", file_stem, uuid, extension);

    if let Some(output_dir) = output_dir {
        output_dir.as_ref().join(filename)
    } else {
        input_path.parent().unwrap_or(Path::new(".")).join(filename)
    }
}

pub fn ensure_output_dir<P: AsRef<Path>>(path: P) -> Result<()> {
    let path = path.as_ref();

    if let Some(parent) = path.parent() {
        if !parent.exists() {
            std::fs::create_dir_all(parent)?;
        }
    }

    Ok(())
}

pub fn get_file_size<P: AsRef<Path>>(path: P) -> Result<u64> {
    let metadata = std::fs::metadata(path)?;
    Ok(metadata.len())
}

pub fn format_file_size(bytes: u64) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];
    const THRESHOLD: f64 = 1024.0;

    if bytes == 0 {
        return "0 B".to_string();
    }

    let size = bytes as f64;
    let unit_index = (size.log(THRESHOLD) as usize).min(UNITS.len() - 1);
    let size_in_unit = size / THRESHOLD.powi(unit_index as i32);

    format!("{:.2} {}", size_in_unit, UNITS[unit_index])
}

#[cfg(test)]
mod tests {
    use super::*;
    // Unused imports removed

    #[test]
    fn test_is_video_file() {
        assert!(is_video_file("test.mkv"));
        assert!(is_video_file("test.MP4"));
        assert!(is_video_file("test.mov"));
        assert!(!is_video_file("test.txt"));
        assert!(!is_video_file("test"));
    }

    #[test]
    fn test_generate_uuid_filename() {
        let input = Path::new("/path/to/movie.mkv");
        let output = generate_uuid_filename(input, None::<&str>);

        assert!(output.to_string_lossy().contains("movie_"));
        assert!(output.to_string_lossy().ends_with(".mkv"));
        assert_eq!(output.parent(), Some(Path::new("/path/to")));
    }

    #[test]
    fn test_format_file_size() {
        assert_eq!(format_file_size(0), "0 B");
        assert_eq!(format_file_size(512), "512.00 B");
        assert_eq!(format_file_size(1024), "1.00 KB");
        assert_eq!(format_file_size(1_048_576), "1.00 MB");
        assert_eq!(format_file_size(1_073_741_824), "1.00 GB");
    }
}
