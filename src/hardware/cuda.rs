use crate::utils::{Result, Error};
use tokio::process::Command;
use tracing::{info, warn, debug};

#[derive(Debug, Clone)]
pub struct HardwareCapabilities {
    pub cuda_available: bool,
    pub nvenc_available: bool,
    pub nvdec_available: bool,
    pub cuda_version: Option<String>,
    pub gpu_devices: Vec<GpuDevice>,
    pub supported_codecs: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct GpuDevice {
    pub index: u32,
    pub name: String,
    pub memory_mb: Option<u32>,
    pub compute_capability: Option<String>,
}

#[derive(Debug, Clone)]
pub enum HardwareAcceleration {
    None,
    CudaDecode,
    CudaDecodeEncode,
    CudaFilter,
    CudaFull, // Decode, filter, encode
}

impl HardwareAcceleration {
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "none" | "disabled" => Self::None,
            "decode" => Self::CudaDecode,
            "encode" => Self::CudaDecodeEncode,
            "filter" => Self::CudaFilter,
            "full" => Self::CudaFull,
            _ => Self::None,
        }
    }
    
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::None => "none",
            Self::CudaDecode => "decode",
            Self::CudaDecodeEncode => "encode",
            Self::CudaFilter => "filter",
            Self::CudaFull => "full",
        }
    }
    
    pub fn uses_decode_acceleration(&self) -> bool {
        matches!(self, Self::CudaDecode | Self::CudaDecodeEncode | Self::CudaFull)
    }
    
    pub fn uses_encode_acceleration(&self) -> bool {
        matches!(self, Self::CudaDecodeEncode | Self::CudaFull)
    }
    
    pub fn uses_filter_acceleration(&self) -> bool {
        matches!(self, Self::CudaFilter | Self::CudaFull)
    }
}

pub struct CudaAccelerator {
    capabilities: HardwareCapabilities,
    fallback_to_software: bool,
}

impl CudaAccelerator {
    pub async fn new(fallback_to_software: bool) -> Result<Self> {
        let capabilities = Self::detect_hardware_capabilities().await?;
        
        info!("CUDA capabilities: available={}, NVENC={}, NVDEC={}", 
              capabilities.cuda_available, 
              capabilities.nvenc_available, 
              capabilities.nvdec_available);
        
        if capabilities.cuda_available {
            info!("Found {} GPU device(s)", capabilities.gpu_devices.len());
            for device in &capabilities.gpu_devices {
                debug!("GPU {}: {} ({}MB)", 
                       device.index, device.name, 
                       device.memory_mb.unwrap_or(0));
            }
        }
        
        Ok(Self {
            capabilities,
            fallback_to_software,
        })
    }
    
    async fn detect_hardware_capabilities() -> Result<HardwareCapabilities> {
        let mut capabilities = HardwareCapabilities {
            cuda_available: false,
            nvenc_available: false,
            nvdec_available: false,
            cuda_version: None,
            gpu_devices: vec![],
            supported_codecs: vec![],
        };
        
        // Check if CUDA is available via nvidia-smi
        if let Ok(cuda_info) = Self::check_cuda_with_nvidia_smi().await {
            capabilities.cuda_available = true;
            capabilities.cuda_version = cuda_info.version;
            capabilities.gpu_devices = cuda_info.devices;
        }
        
        // Check FFmpeg hardware acceleration support
        let ffmpeg_capabilities = Self::check_ffmpeg_hardware_support().await?;
        capabilities.nvenc_available = ffmpeg_capabilities.nvenc_available;
        capabilities.nvdec_available = ffmpeg_capabilities.nvdec_available;
        capabilities.supported_codecs = ffmpeg_capabilities.supported_codecs;
        
        Ok(capabilities)
    }
    
    async fn check_cuda_with_nvidia_smi() -> Result<CudaSystemInfo> {
        let output = Command::new("nvidia-smi")
            .args(&["--query-gpu=index,name,memory.total,driver_version", "--format=csv,noheader,nounits"])
            .output()
            .await;
        
        match output {
            Ok(result) if result.status.success() => {
                let stdout = String::from_utf8_lossy(&result.stdout);
                let mut devices = Vec::new();
                let mut driver_version = None;
                
                for line in stdout.lines() {
                    let parts: Vec<&str> = line.split(',').map(|s| s.trim()).collect();
                    if parts.len() >= 4 {
                        let index = parts[0].parse().unwrap_or(0);
                        let name = parts[1].to_string();
                        let memory_mb = parts[2].parse().ok();
                        
                        if driver_version.is_none() {
                            driver_version = Some(parts[3].to_string());
                        }
                        
                        devices.push(GpuDevice {
                            index,
                            name,
                            memory_mb,
                            compute_capability: None, // Would need additional query
                        });
                    }
                }
                
                Ok(CudaSystemInfo {
                    version: driver_version,
                    devices,
                })
            },
            _ => Err(Error::hardware("CUDA not available - nvidia-smi not found or failed")),
        }
    }
    
    async fn check_ffmpeg_hardware_support() -> Result<FfmpegHardwareInfo> {
        // Check for hardware decoders
        let decoders_output = Command::new("ffmpeg")
            .args(&["-hide_banner", "-decoders"])
            .output()
            .await?;
        
        let decoders_text = String::from_utf8_lossy(&decoders_output.stdout);
        let nvdec_available = decoders_text.contains("h264_nvdec") || decoders_text.contains("hevc_nvdec");
        
        // Check for hardware encoders
        let encoders_output = Command::new("ffmpeg")
            .args(&["-hide_banner", "-encoders"])
            .output()
            .await?;
        
        let encoders_text = String::from_utf8_lossy(&encoders_output.stdout);
        let nvenc_available = encoders_text.contains("h264_nvenc") || encoders_text.contains("hevc_nvenc");
        
        // Extract supported codecs
        let mut supported_codecs = Vec::new();
        if nvdec_available {
            if decoders_text.contains("h264_nvdec") {
                supported_codecs.push("h264_nvdec".to_string());
            }
            if decoders_text.contains("hevc_nvdec") {
                supported_codecs.push("hevc_nvdec".to_string());
            }
        }
        if nvenc_available {
            if encoders_text.contains("h264_nvenc") {
                supported_codecs.push("h264_nvenc".to_string());
            }
            if encoders_text.contains("hevc_nvenc") {
                supported_codecs.push("hevc_nvenc".to_string());
            }
        }
        
        Ok(FfmpegHardwareInfo {
            nvenc_available,
            nvdec_available,
            supported_codecs,
        })
    }
    
    pub fn get_capabilities(&self) -> &HardwareCapabilities {
        &self.capabilities
    }
    
    pub fn build_hardware_args(
        &self,
        acceleration_level: HardwareAcceleration,
        input_codec: Option<&str>,
    ) -> Result<Vec<String>> {
        if !self.capabilities.cuda_available && !self.fallback_to_software {
            return Err(Error::hardware("CUDA not available and fallback disabled"));
        }
        
        if matches!(acceleration_level, HardwareAcceleration::None) {
            return Ok(vec![]);
        }
        
        let mut args = Vec::new();
        
        // Add hardware decode acceleration
        if acceleration_level.uses_decode_acceleration() && self.capabilities.nvdec_available {
            if let Some(codec) = input_codec {
                match codec.to_lowercase().as_str() {
                    "h264" => {
                        args.extend(vec![
                            "-hwaccel".to_string(),
                            "cuda".to_string(),
                            "-hwaccel_output_format".to_string(),
                            "cuda".to_string(),
                            "-c:v".to_string(),
                            "h264_nvdec".to_string(),
                        ]);
                    },
                    "hevc" | "h265" => {
                        args.extend(vec![
                            "-hwaccel".to_string(),
                            "cuda".to_string(),
                            "-hwaccel_output_format".to_string(),
                            "cuda".to_string(),
                            "-c:v".to_string(),
                            "hevc_nvdec".to_string(),
                        ]);
                    },
                    _ => {
                        // Generic CUDA acceleration for other codecs
                        args.extend(vec![
                            "-hwaccel".to_string(),
                            "cuda".to_string(),
                        ]);
                    }
                }
            } else {
                // Generic CUDA acceleration
                args.extend(vec![
                    "-hwaccel".to_string(),
                    "cuda".to_string(),
                ]);
            }
            
            debug!("Using CUDA decode acceleration");
        }
        
        Ok(args)
    }
    
    pub fn build_cuda_filter_chain(&self, base_filters: &[String]) -> Result<Vec<String>> {
        if !self.capabilities.cuda_available {
            return Ok(base_filters.to_vec());
        }
        
        let mut cuda_filters = Vec::new();
        let needs_hwupload = true;
        let mut needs_hwdownload = false;
        
        // Start with hwupload if not already on GPU
        if needs_hwupload {
            cuda_filters.push("hwupload_cuda".to_string());
        }
        
        // Convert software filters to CUDA equivalents where possible
        for filter in base_filters {
            let cuda_filter = match filter.as_str() {
                filter if filter.starts_with("scale=") => {
                    needs_hwdownload = true;
                    format!("scale_cuda={}", &filter[6..])
                },
                filter if filter.starts_with("yadif") => {
                    needs_hwdownload = true;
                    // CUDA yadif equivalent
                    "yadif_cuda".to_string()
                },
                filter if filter.starts_with("hqdn3d") => {
                    needs_hwdownload = true;
                    // Use software denoising, will need hwdownload/hwupload
                    filter.to_string()
                },
                _ => {
                    // Keep software filter as-is
                    needs_hwdownload = true;
                    filter.to_string()
                }
            };
            cuda_filters.push(cuda_filter);
        }
        
        // Add hwdownload if we need to return to software
        if needs_hwdownload {
            cuda_filters.push("hwdownload".to_string());
            cuda_filters.push("format=nv12".to_string());
        }
        
        debug!("CUDA filter chain: {:?}", cuda_filters);
        Ok(cuda_filters)
    }
    
    pub async fn test_cuda_pipeline(&self) -> Result<bool> {
        if !self.capabilities.cuda_available {
            return Ok(false);
        }
        
        // Test basic CUDA functionality with a simple filter
        let output = Command::new("ffmpeg")
            .args(&[
                "-f", "lavfi",
                "-i", "testsrc=duration=1:size=320x240:rate=1",
                "-vf", "hwupload_cuda,scale_cuda=160:120,hwdownload,format=yuv420p",
                "-f", "null",
                "-",
            ])
            .output()
            .await?;
        
        let success = output.status.success();
        
        if success {
            info!("CUDA pipeline test passed");
        } else {
            let error = String::from_utf8_lossy(&output.stderr);
            warn!("CUDA pipeline test failed: {}", error);
        }
        
        Ok(success)
    }
    
    pub fn get_optimal_acceleration_level(&self) -> HardwareAcceleration {
        if !self.capabilities.cuda_available {
            return HardwareAcceleration::None;
        }
        
        if self.capabilities.nvenc_available && self.capabilities.nvdec_available {
            HardwareAcceleration::CudaFull
        } else if self.capabilities.nvdec_available {
            HardwareAcceleration::CudaDecode
        } else {
            HardwareAcceleration::None
        }
    }
}

#[derive(Debug)]
struct CudaSystemInfo {
    version: Option<String>,
    devices: Vec<GpuDevice>,
}

#[derive(Debug)]
struct FfmpegHardwareInfo {
    nvenc_available: bool,
    nvdec_available: bool,
    supported_codecs: Vec<String>,
}

impl Default for CudaAccelerator {
    fn default() -> Self {
        Self {
            capabilities: HardwareCapabilities {
                cuda_available: false,
                nvenc_available: false,
                nvdec_available: false,
                cuda_version: None,
                gpu_devices: vec![],
                supported_codecs: vec![],
            },
            fallback_to_software: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hardware_acceleration_from_str() {
        assert!(matches!(HardwareAcceleration::from_str("none"), HardwareAcceleration::None));
        assert!(matches!(HardwareAcceleration::from_str("decode"), HardwareAcceleration::CudaDecode));
        assert!(matches!(HardwareAcceleration::from_str("full"), HardwareAcceleration::CudaFull));
    }

    #[test]
    fn test_acceleration_level_checks() {
        let full = HardwareAcceleration::CudaFull;
        assert!(full.uses_decode_acceleration());
        assert!(full.uses_encode_acceleration());
        assert!(full.uses_filter_acceleration());
        
        let none = HardwareAcceleration::None;
        assert!(!none.uses_decode_acceleration());
        assert!(!none.uses_encode_acceleration());
        assert!(!none.uses_filter_acceleration());
    }

    #[tokio::test]
    async fn test_cuda_accelerator_creation() {
        // This test may fail on systems without CUDA, which is expected
        let result = CudaAccelerator::new(true).await;
        assert!(result.is_ok());
    }
}