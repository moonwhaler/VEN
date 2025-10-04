# Modular AV1 Development Plan

**Version:** 1.0
**Date:** 2025-10-04
**Status:** Planning Phase

---

## Table of Contents

1. [Overview](#overview)
2. [Current Architecture Analysis](#current-architecture-analysis)
3. [Modular AV1 Integration Architecture](#modular-av1-integration-architecture)
4. [Implementation Plan](#implementation-plan)
5. [Configuration Changes](#configuration-changes)
6. [File Structure Changes](#file-structure-changes)
7. [Testing Strategy](#testing-strategy)
8. [Key Advantages](#key-advantages)
9. [Codec Parameter Mapping Reference](#codec-parameter-mapping-reference)
10. [Summary](#summary)

---

## Overview

This document outlines a comprehensive plan to integrate AV1 encoding support into the FFmpeg autoencoder while maintaining **100% backwards compatibility** with existing x265 workflows. The design follows clean architecture principles with modular, scalable, and maintainable code.

### Design Goals

1. **Zero Breaking Changes**: All existing x265 functionality must work identically
2. **Codec Abstraction**: Create codec-agnostic interfaces
3. **Configuration-Driven**: Codec selection via config, not code
4. **Extensibility**: Easy to add more codecs in future (VP9, H.264, etc.)
5. **Parameter Isolation**: Codec-specific params in separate namespaces

---

## Current Architecture Analysis

### Codec Coupling Points Identified

#### 1. Hardcoded Codec Selection (`src/encoding/modes.rs`)

- **Line 117**: `args.extend(vec!["-c:v".to_string(), "libx265".to_string()]);` (CrfEncoder)
- **Line 343**: `args.extend(vec!["-c:v".to_string(), "libx265".to_string()]);` (AbrEncoder pass 1)
- **Line 441**: `args.extend(vec!["-c:v".to_string(), "libx265".to_string()]);` (AbrEncoder pass 2)

All three encoding modes (CRF, ABR, CBR) hardcode `libx265` as the video codec.

#### 2. Parameter Structure (`src/config/`)

**File: `src/config/profiles.rs:15`**
```rust
pub struct EncodingProfile {
    // ...
    pub x265_params: HashMap<String, String>,
}
```

All parameter building methods are x265-specific:
- `build_x265_params_string()` (profiles.rs:71-84)
- `build_x265_params_string_with_hdr()` (profiles.rs:99-119)
- `build_x265_params_string_with_dolby_vision()` (profiles.rs:324-437)

#### 3. HDR Parameter Building (`src/hdr/encoding.rs`)

**File: `src/hdr/encoding.rs:10`**
```rust
pub fn build_hdr_x265_params(
    &self,
    hdr_metadata: &HdrMetadata,
    base_params: &HashMap<String, String>,
) -> HashMap<String, String>
```

All HDR metadata formatting is x265-specific:
- `format_master_display_for_x265`
- Parameter names: `colorprim`, `transfer`, `colormatrix`, `master-display`, `max-cll`

#### 4. Configuration Files

All profiles use `x265_params` section in YAML:
```yaml
profiles:
  movie:
    x265_params:
      preset: "slow"
      pix_fmt: "yuv420p10le"
      profile: "main10"
      # ... all x265-specific
```

---

## Modular AV1 Integration Architecture

### Architecture Overview

```
┌─────────────────────────────────────────────────────────────┐
│                    Configuration Layer                       │
│  • Codec specification per profile                          │
│  • Codec-specific parameter namespaces                      │
└─────────────────┬───────────────────────────────────────────┘
                  │
┌─────────────────▼───────────────────────────────────────────┐
│                 Codec Abstraction Layer                      │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐     │
│  │ VideoCodec   │  │ CodecParams  │  │ CodecFactory │     │
│  │   (trait)    │  │   (trait)    │  │              │     │
│  └──────────────┘  └──────────────┘  └──────────────┘     │
└─────────────────┬───────────────────────────────────────────┘
                  │
┌─────────────────▼───────────────────────────────────────────┐
│              Codec Implementations                           │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐     │
│  │ X265Codec    │  │ Av1Codec     │  │ Svt-Av1Codec │     │
│  │ (libx265)    │  │ (libaom-av1) │  │ (libsvtav1)  │     │
│  └──────────────┘  └──────────────┘  └──────────────┘     │
└─────────────────┬───────────────────────────────────────────┘
                  │
┌─────────────────▼───────────────────────────────────────────┐
│            HDR Parameter Translation                         │
│  • Codec-specific HDR metadata formatting                   │
│  • BT.2020, PQ, HLG parameter mapping                       │
│  • Dolby Vision / HDR10+ support per codec                  │
└─────────────────────────────────────────────────────────────┘
```

### Key Components

1. **Configuration Layer**: YAML-based codec selection and parameters
2. **Codec Abstraction**: Traits defining codec capabilities
3. **Codec Factory**: Dynamic codec registration and retrieval
4. **Codec Implementations**: Separate modules for each codec
5. **HDR Translation**: Codec-specific metadata formatting

---

## Implementation Plan

### Phase 1: Core Abstraction Layer

#### 1.1 Create Codec Trait (`src/encoding/codec/mod.rs` - NEW)

```rust
use std::collections::HashMap;
use crate::utils::{Result, Error};
use crate::utils::ffmpeg::VideoMetadata;

/// Trait defining video codec capabilities
pub trait VideoCodec: Send + Sync {
    /// Get codec identifier (e.g., "x265", "av1-aom", "svt-av1")
    fn id(&self) -> &str;

    /// Get FFmpeg codec name (e.g., "libx265", "libaom-av1", "libsvtav1")
    fn ffmpeg_name(&self) -> &str;

    /// Get codec display name
    fn display_name(&self) -> &str;

    /// Build codec-specific ffmpeg arguments
    fn build_ffmpeg_args(
        &self,
        params: &dyn CodecParams,
        mode_params: &HashMap<String, String>,
        metadata: &VideoMetadata,
    ) -> Result<Vec<String>>;

    /// Validate codec availability in FFmpeg
    fn validate_availability(&self, ffmpeg: &FfmpegWrapper) -> Result<bool>;

    /// Check if codec supports HDR
    fn supports_hdr(&self) -> bool;

    /// Check if codec supports Dolby Vision
    fn supports_dolby_vision(&self) -> bool;

    /// Check if codec supports HDR10+
    fn supports_hdr10_plus(&self) -> bool;

    /// Get optimal CRF range for this codec
    fn crf_range(&self) -> (f32, f32);

    /// Map generic quality preset to codec-specific preset
    fn map_preset(&self, generic_preset: &str) -> String;
}

/// Trait for codec-specific parameters
pub trait CodecParams: Send + Sync {
    /// Get codec-specific parameter string (e.g., x265-params format)
    fn as_param_string(&self) -> String;

    /// Get preset value
    fn get_preset(&self) -> Option<String>;

    /// Get profile value
    fn get_profile(&self) -> Option<String>;

    /// Get pixel format
    fn get_pixel_format(&self) -> Option<String>;

    /// Inject HDR metadata
    fn with_hdr_metadata(&mut self, metadata: &HdrMetadata);

    /// Inject mode-specific params (CRF, bitrate, etc.)
    fn with_mode_params(&mut self, params: &HashMap<String, String>);

    /// Clone parameters
    fn clone_params(&self) -> Box<dyn CodecParams>;
}
```

#### 1.2 Create Codec Factory (`src/encoding/codec/factory.rs` - NEW)

```rust
use std::collections::HashMap;
use super::{VideoCodec, X265Codec, Av1AomCodec, SvtAv1Codec};
use crate::utils::{Result, Error};

pub struct CodecFactory {
    codecs: HashMap<String, Box<dyn VideoCodec>>,
}

impl CodecFactory {
    pub fn new() -> Self {
        let mut factory = Self {
            codecs: HashMap::new(),
        };

        // Register available codecs
        factory.register(Box::new(X265Codec::new()));
        factory.register(Box::new(Av1AomCodec::new()));
        factory.register(Box::new(SvtAv1Codec::new()));

        factory
    }

    pub fn register(&mut self, codec: Box<dyn VideoCodec>) {
        self.codecs.insert(codec.id().to_string(), codec);
    }

    pub fn get_codec(&self, id: &str) -> Result<&dyn VideoCodec> {
        self.codecs.get(id)
            .map(|c| c.as_ref())
            .ok_or_else(|| Error::codec(format!("Codec '{}' not found", id)))
    }

    pub fn list_codecs(&self) -> Vec<String> {
        self.codecs.keys().cloned().collect()
    }

    pub fn validate_codec(&self, id: &str, ffmpeg: &FfmpegWrapper) -> Result<bool> {
        let codec = self.get_codec(id)?;
        codec.validate_availability(ffmpeg)
    }
}

impl Default for CodecFactory {
    fn default() -> Self {
        Self::new()
    }
}
```

---

### Phase 2: X265 Implementation (Refactor Existing)

#### 2.1 X265Codec Implementation (`src/encoding/codec/x265.rs` - NEW)

```rust
use super::{VideoCodec, CodecParams};
use crate::utils::{Result, FfmpegWrapper};
use crate::utils::ffmpeg::VideoMetadata;
use crate::hdr::HdrMetadata;
use std::collections::HashMap;

pub struct X265Codec;

impl X265Codec {
    pub fn new() -> Self {
        Self
    }
}

impl VideoCodec for X265Codec {
    fn id(&self) -> &str { "x265" }

    fn ffmpeg_name(&self) -> &str { "libx265" }

    fn display_name(&self) -> &str { "HEVC (x265)" }

    fn build_ffmpeg_args(
        &self,
        params: &dyn CodecParams,
        mode_params: &HashMap<String, String>,
        metadata: &VideoMetadata,
    ) -> Result<Vec<String>> {
        let mut args = vec!["-c:v".to_string(), "libx265".to_string()];

        if let Some(preset) = params.get_preset() {
            args.extend(vec!["-preset".to_string(), preset]);
        }

        if let Some(profile) = params.get_profile() {
            args.extend(vec!["-profile:v".to_string(), profile]);
        }

        if let Some(pix_fmt) = params.get_pixel_format() {
            args.extend(vec!["-pix_fmt".to_string(), pix_fmt]);
        }

        // Build x265-params string
        args.extend(vec!["-x265-params".to_string(), params.as_param_string()]);

        Ok(args)
    }

    fn validate_availability(&self, ffmpeg: &FfmpegWrapper) -> Result<bool> {
        // Check if libx265 is available in FFmpeg
        // Implementation: run `ffmpeg -codecs | grep libx265`
        Ok(true) // Placeholder
    }

    fn supports_hdr(&self) -> bool { true }
    fn supports_dolby_vision(&self) -> bool { true }
    fn supports_hdr10_plus(&self) -> bool { true }

    fn crf_range(&self) -> (f32, f32) { (0.0, 51.0) }

    fn map_preset(&self, generic: &str) -> String {
        // x265 presets: ultrafast, superfast, veryfast, faster, fast, medium,
        // slow, slower, veryslow, placebo
        generic.to_string() // Direct mapping for x265
    }
}

#[derive(Clone)]
pub struct X265Params {
    params: HashMap<String, String>,
    preset: Option<String>,
    profile: Option<String>,
    pix_fmt: Option<String>,
}

impl X265Params {
    pub fn new() -> Self {
        Self {
            params: HashMap::new(),
            preset: None,
            profile: None,
            pix_fmt: None,
        }
    }

    pub fn from_hashmap(
        params: HashMap<String, String>,
        preset: Option<String>,
        profile: Option<String>,
        pix_fmt: Option<String>,
    ) -> Self {
        Self {
            params,
            preset,
            profile,
            pix_fmt,
        }
    }
}

impl CodecParams for X265Params {
    fn as_param_string(&self) -> String {
        let param_strs: Vec<String> = self.params
            .iter()
            .map(|(k, v)| {
                if v.is_empty() || v == "true" {
                    k.clone()
                } else {
                    format!("{}={}", k, v)
                }
            })
            .collect();
        param_strs.join(":")
    }

    fn get_preset(&self) -> Option<String> { self.preset.clone() }

    fn get_profile(&self) -> Option<String> { self.profile.clone() }

    fn get_pixel_format(&self) -> Option<String> { self.pix_fmt.clone() }

    fn with_hdr_metadata(&mut self, metadata: &HdrMetadata) {
        // Existing x265 HDR parameter logic
        self.params.insert("colorprim".to_string(), "bt2020".to_string());
        self.params.insert("transfer".to_string(), "smpte2084".to_string());
        self.params.insert("colormatrix".to_string(), "bt2020nc".to_string());

        if let Some(ref md) = metadata.master_display {
            let md_string = format!(
                "G({},{})B({},{})R({},{})WP({},{})L({},{})",
                md.display_primaries_g_x, md.display_primaries_g_y,
                md.display_primaries_b_x, md.display_primaries_b_y,
                md.display_primaries_r_x, md.display_primaries_r_y,
                md.white_point_x, md.white_point_y,
                md.max_luminance, md.min_luminance
            );
            self.params.insert("master-display".to_string(), md_string);
        }

        if let Some(ref cll) = metadata.content_light_level {
            let cll_string = format!("{},{}", cll.max_cll, cll.max_fall);
            self.params.insert("max-cll".to_string(), cll_string);
        }

        self.params.insert("hdr10_opt".to_string(), "1".to_string());
    }

    fn with_mode_params(&mut self, params: &HashMap<String, String>) {
        for (k, v) in params {
            self.params.insert(k.clone(), v.clone());
        }
    }

    fn clone_params(&self) -> Box<dyn CodecParams> {
        Box::new(self.clone())
    }
}
```

---

### Phase 3: AV1 Codec Implementations

#### 3.1 AV1 (libaom-av1) Implementation (`src/encoding/codec/av1_aom.rs` - NEW)

```rust
use super::{VideoCodec, CodecParams};
use crate::utils::{Result, FfmpegWrapper};
use crate::utils::ffmpeg::VideoMetadata;
use crate::hdr::HdrMetadata;
use std::collections::HashMap;

pub struct Av1AomCodec;

impl Av1AomCodec {
    pub fn new() -> Self {
        Self
    }
}

impl VideoCodec for Av1AomCodec {
    fn id(&self) -> &str { "av1-aom" }

    fn ffmpeg_name(&self) -> &str { "libaom-av1" }

    fn display_name(&self) -> &str { "AV1 (libaom)" }

    fn build_ffmpeg_args(
        &self,
        params: &dyn CodecParams,
        mode_params: &HashMap<String, String>,
        metadata: &VideoMetadata,
    ) -> Result<Vec<String>> {
        let mut args = vec!["-c:v".to_string(), "libaom-av1".to_string()];

        if let Some(preset) = params.get_preset() {
            // libaom-av1 uses -cpu-used parameter (0-8, lower = slower/better)
            args.extend(vec!["-cpu-used".to_string(), preset]);
        }

        if let Some(pix_fmt) = params.get_pixel_format() {
            args.extend(vec!["-pix_fmt".to_string(), pix_fmt]);
        }

        // AV1 uses individual parameters, not a params string
        self.add_av1_params(&mut args, params)?;

        Ok(args)
    }

    fn validate_availability(&self, ffmpeg: &FfmpegWrapper) -> Result<bool> {
        // Check if libaom-av1 is available
        Ok(true) // Placeholder
    }

    fn supports_hdr(&self) -> bool { true }
    fn supports_dolby_vision(&self) -> bool { false } // AV1 doesn't support DV RPU
    fn supports_hdr10_plus(&self) -> bool { true }

    fn crf_range(&self) -> (f32, f32) { (0.0, 63.0) } // AV1 CRF range

    fn map_preset(&self, generic: &str) -> String {
        // Map x265-style presets to cpu-used values (0-8)
        match generic {
            "ultrafast" | "superfast" => "8",
            "veryfast" => "7",
            "faster" => "6",
            "fast" => "5",
            "medium" => "4",
            "slow" => "3",
            "slower" => "2",
            "veryslow" => "1",
            "placebo" => "0",
            _ => "4", // default to medium
        }.to_string()
    }
}

impl Av1AomCodec {
    fn add_av1_params(&self, args: &mut Vec<String>, params: &dyn CodecParams) -> Result<()> {
        // AV1-specific parameter handling
        // Individual FFmpeg arguments instead of params string

        // Example: -aq-mode, -enable-cdef, etc.
        // These would be extracted from params and added to args

        Ok(())
    }
}

#[derive(Clone)]
pub struct Av1AomParams {
    cpu_used: String,
    crf: Option<f32>,
    params: HashMap<String, String>,
    pix_fmt: Option<String>,
}

impl Av1AomParams {
    pub fn new(cpu_used: String) -> Self {
        Self {
            cpu_used,
            crf: None,
            params: HashMap::new(),
            pix_fmt: Some("yuv420p10le".to_string()),
        }
    }

    pub fn from_hashmap(
        cpu_used: String,
        params: HashMap<String, String>,
        pix_fmt: Option<String>,
    ) -> Self {
        Self {
            cpu_used,
            crf: None,
            params,
            pix_fmt,
        }
    }

    fn format_master_display_av1(&self, md: &crate::hdr::MasteringDisplayMetadata) -> String {
        // AV1 mastering display format
        format!(
            "{}:{}:{}:{}:{}:{}:{}:{}:{}:{}",
            md.display_primaries_g_x, md.display_primaries_g_y,
            md.display_primaries_b_x, md.display_primaries_b_y,
            md.display_primaries_r_x, md.display_primaries_r_y,
            md.white_point_x, md.white_point_y,
            md.max_luminance, md.min_luminance
        )
    }
}

impl CodecParams for Av1AomParams {
    fn as_param_string(&self) -> String {
        // Not used for AV1 - parameters are passed individually
        String::new()
    }

    fn get_preset(&self) -> Option<String> {
        Some(self.cpu_used.clone())
    }

    fn get_profile(&self) -> Option<String> { None }

    fn get_pixel_format(&self) -> Option<String> {
        self.pix_fmt.clone()
    }

    fn with_hdr_metadata(&mut self, metadata: &HdrMetadata) {
        // AV1 HDR parameter mapping (different names than x265)
        self.params.insert("color-primaries".to_string(), "bt2020".to_string());
        self.params.insert("transfer-characteristics".to_string(), "smpte2084".to_string());
        self.params.insert("matrix-coefficients".to_string(), "bt2020nc".to_string());

        // Add mastering display metadata
        if let Some(ref md) = metadata.master_display {
            let md_str = self.format_master_display_av1(md);
            self.params.insert("mastering-display".to_string(), md_str);
        }

        // Add content light level
        if let Some(ref cll) = metadata.content_light_level {
            let cll_str = format!("{},{}", cll.max_cll, cll.max_fall);
            self.params.insert("content-light-level".to_string(), cll_str);
        }
    }

    fn with_mode_params(&mut self, params: &HashMap<String, String>) {
        for (k, v) in params {
            self.params.insert(k.clone(), v.clone());
        }
    }

    fn clone_params(&self) -> Box<dyn CodecParams> {
        Box::new(self.clone())
    }
}
```

#### 3.2 SVT-AV1 Implementation (`src/encoding/codec/svt_av1.rs` - NEW)

```rust
use super::{VideoCodec, CodecParams};
use crate::utils::{Result, FfmpegWrapper};
use crate::utils::ffmpeg::VideoMetadata;
use crate::hdr::HdrMetadata;
use std::collections::HashMap;

pub struct SvtAv1Codec;

impl SvtAv1Codec {
    pub fn new() -> Self {
        Self
    }
}

impl VideoCodec for SvtAv1Codec {
    fn id(&self) -> &str { "svt-av1" }

    fn ffmpeg_name(&self) -> &str { "libsvtav1" }

    fn display_name(&self) -> &str { "AV1 (SVT-AV1)" }

    fn build_ffmpeg_args(
        &self,
        params: &dyn CodecParams,
        mode_params: &HashMap<String, String>,
        metadata: &VideoMetadata,
    ) -> Result<Vec<String>> {
        let mut args = vec!["-c:v".to_string(), "libsvtav1".to_string()];

        if let Some(preset) = params.get_preset() {
            // SVT-AV1 uses -preset parameter (0-13, lower = slower/better)
            args.extend(vec!["-preset".to_string(), preset]);
        }

        if let Some(pix_fmt) = params.get_pixel_format() {
            args.extend(vec!["-pix_fmt".to_string(), pix_fmt]);
        }

        // SVT-AV1 uses -svtav1-params
        let svtav1_params = self.build_svtav1_params(params)?;
        if !svtav1_params.is_empty() {
            args.extend(vec!["-svtav1-params".to_string(), svtav1_params]);
        }

        Ok(args)
    }

    fn validate_availability(&self, ffmpeg: &FfmpegWrapper) -> Result<bool> {
        // Check if libsvtav1 is available
        Ok(true) // Placeholder
    }

    fn supports_hdr(&self) -> bool { true }
    fn supports_dolby_vision(&self) -> bool { false }
    fn supports_hdr10_plus(&self) -> bool { true }

    fn crf_range(&self) -> (f32, f32) { (0.0, 63.0) }

    fn map_preset(&self, generic: &str) -> String {
        // Map to SVT-AV1 preset range (0-13)
        match generic {
            "ultrafast" | "superfast" => "12",
            "veryfast" => "10",
            "faster" => "8",
            "fast" => "7",
            "medium" => "6",
            "slow" => "4",
            "slower" => "3",
            "veryslow" => "2",
            "placebo" => "0",
            _ => "6",
        }.to_string()
    }
}

impl SvtAv1Codec {
    fn build_svtav1_params(&self, params: &dyn CodecParams) -> Result<String> {
        // Build svtav1-params string (similar to x265-params format)
        Ok(params.as_param_string())
    }
}

#[derive(Clone)]
pub struct SvtAv1Params {
    preset: String,
    params: HashMap<String, String>,
    pix_fmt: Option<String>,
}

impl SvtAv1Params {
    pub fn new(preset: String) -> Self {
        Self {
            preset,
            params: HashMap::new(),
            pix_fmt: Some("yuv420p10le".to_string()),
        }
    }

    pub fn from_hashmap(
        preset: String,
        params: HashMap<String, String>,
        pix_fmt: Option<String>,
    ) -> Self {
        Self {
            preset,
            params,
            pix_fmt,
        }
    }
}

impl CodecParams for SvtAv1Params {
    fn as_param_string(&self) -> String {
        let param_strs: Vec<String> = self.params
            .iter()
            .map(|(k, v)| {
                if v.is_empty() || v == "true" {
                    k.clone()
                } else {
                    format!("{}={}", k, v)
                }
            })
            .collect();
        param_strs.join(":")
    }

    fn get_preset(&self) -> Option<String> {
        Some(self.preset.clone())
    }

    fn get_profile(&self) -> Option<String> { None }

    fn get_pixel_format(&self) -> Option<String> {
        self.pix_fmt.clone()
    }

    fn with_hdr_metadata(&mut self, metadata: &HdrMetadata) {
        // SVT-AV1 HDR parameter mapping
        self.params.insert("color-primaries".to_string(), "bt2020".to_string());
        self.params.insert("transfer-characteristics".to_string(), "smpte2084".to_string());
        self.params.insert("matrix-coefficients".to_string(), "bt2020nc".to_string());

        if let Some(ref md) = metadata.master_display {
            let md_str = format!(
                "{}:{}:{}:{}:{}:{}:{}:{}:{}:{}",
                md.display_primaries_g_x, md.display_primaries_g_y,
                md.display_primaries_b_x, md.display_primaries_b_y,
                md.display_primaries_r_x, md.display_primaries_r_y,
                md.white_point_x, md.white_point_y,
                md.max_luminance, md.min_luminance
            );
            self.params.insert("mastering-display".to_string(), md_str);
        }

        if let Some(ref cll) = metadata.content_light_level {
            let cll_str = format!("{},{}", cll.max_cll, cll.max_fall);
            self.params.insert("content-light-level".to_string(), cll_str);
        }
    }

    fn with_mode_params(&mut self, params: &HashMap<String, String>) {
        for (k, v) in params {
            self.params.insert(k.clone(), v.clone());
        }
    }

    fn clone_params(&self) -> Box<dyn CodecParams> {
        Box::new(self.clone())
    }
}
```

---

## Configuration Changes

### Phase 4: Update Profile Structure

#### 4.1 Update `src/config/types.rs`

```rust
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RawProfile {
    pub title: String,
    pub base_crf: f32,
    pub bitrate: u32,
    pub content_type: String,

    // NEW: Codec specification
    #[serde(default = "default_codec")]
    pub codec: String, // "x265", "av1-aom", "svt-av1"

    // Legacy x265 params (kept for backwards compatibility)
    #[serde(default)]
    pub x265_params: HashMap<String, serde_yaml::Value>,

    // NEW: Codec-agnostic params
    #[serde(default)]
    pub av1_params: Option<HashMap<String, serde_yaml::Value>>,

    #[serde(default)]
    pub svtav1_params: Option<HashMap<String, serde_yaml::Value>>,
}

fn default_codec() -> String {
    "x265".to_string() // Default to x265 for backwards compatibility
}
```

#### 4.2 Update `src/config/profiles.rs`

```rust
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EncodingProfile {
    pub name: String,
    pub title: String,
    pub base_crf: f32,
    pub bitrate: u32,
    pub content_type: ContentType,

    // NEW: Codec information
    pub codec: String,
    pub codec_params: Box<dyn CodecParams>, // Trait object

    // DEPRECATED but kept for compatibility
    pub x265_params: HashMap<String, String>,
}

impl EncodingProfile {
    pub fn from_raw(
        name: String,
        raw: RawProfile,
        codec_factory: &CodecFactory
    ) -> Result<Self> {
        let content_type = ContentType::from_string(&raw.content_type)
            .ok_or_else(|| Error::profile(format!("Invalid content type: {}", raw.content_type)))?;

        // Get codec from factory
        let codec = codec_factory.get_codec(&raw.codec)?;

        // Build codec-specific parameters
        let codec_params = match raw.codec.as_str() {
            "x265" => Self::build_x265_params(&raw.x265_params)?,
            "av1-aom" => Self::build_av1_aom_params(raw.av1_params.as_ref())?,
            "svt-av1" => Self::build_svtav1_params(raw.svtav1_params.as_ref())?,
            _ => return Err(Error::profile(format!("Unknown codec: {}", raw.codec))),
        };

        Ok(EncodingProfile {
            name,
            title: raw.title,
            base_crf: raw.base_crf,
            bitrate: raw.bitrate,
            content_type,
            codec: raw.codec,
            codec_params,
            x265_params: HashMap::new(), // Legacy compatibility
        })
    }

    fn build_x265_params(raw: &HashMap<String, serde_yaml::Value>) -> Result<Box<dyn CodecParams>> {
        let mut params = HashMap::new();
        let mut preset = None;
        let mut profile = None;
        let mut pix_fmt = None;

        for (key, value) in raw {
            match key.as_str() {
                "preset" => preset = Some(Self::yaml_value_to_string(value)?),
                "profile" => profile = Some(Self::yaml_value_to_string(value)?),
                "pix_fmt" => pix_fmt = Some(Self::yaml_value_to_string(value)?),
                _ => {
                    params.insert(key.clone(), Self::yaml_value_to_string(value)?);
                }
            }
        }

        Ok(Box::new(X265Params::from_hashmap(params, preset, profile, pix_fmt)))
    }

    fn build_av1_aom_params(raw: Option<&HashMap<String, serde_yaml::Value>>) -> Result<Box<dyn CodecParams>> {
        let raw = raw.ok_or_else(|| Error::profile("av1_params required for av1-aom codec"))?;

        let cpu_used = raw.get("cpu_used")
            .and_then(|v| v.as_i64())
            .unwrap_or(4)
            .to_string();

        let pix_fmt = raw.get("pix_fmt")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let mut params = HashMap::new();
        for (key, value) in raw {
            if key != "cpu_used" && key != "pix_fmt" {
                params.insert(key.clone(), Self::yaml_value_to_string(value)?);
            }
        }

        Ok(Box::new(Av1AomParams::from_hashmap(cpu_used, params, pix_fmt)))
    }

    fn build_svtav1_params(raw: Option<&HashMap<String, serde_yaml::Value>>) -> Result<Box<dyn CodecParams>> {
        let raw = raw.ok_or_else(|| Error::profile("svtav1_params required for svt-av1 codec"))?;

        let preset = raw.get("preset")
            .and_then(|v| v.as_i64())
            .unwrap_or(6)
            .to_string();

        let pix_fmt = raw.get("pix_fmt")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let mut params = HashMap::new();
        for (key, value) in raw {
            if key != "preset" && key != "pix_fmt" {
                params.insert(key.clone(), Self::yaml_value_to_string(value)?);
            }
        }

        Ok(Box::new(SvtAv1Params::from_hashmap(preset, params, pix_fmt)))
    }

    fn yaml_value_to_string(value: &serde_yaml::Value) -> Result<String> {
        match value {
            serde_yaml::Value::String(s) => Ok(s.clone()),
            serde_yaml::Value::Number(n) => Ok(n.to_string()),
            serde_yaml::Value::Bool(b) => Ok(if *b { "1" } else { "0" }.to_string()),
            _ => Err(Error::profile(format!("Unsupported YAML value: {:?}", value))),
        }
    }

    // NEW: Generic parameter building
    pub fn build_codec_args(
        &self,
        codec_factory: &CodecFactory,
        mode_params: &HashMap<String, String>,
        metadata: &VideoMetadata,
    ) -> Result<Vec<String>> {
        let codec = codec_factory.get_codec(&self.codec)?;
        codec.build_ffmpeg_args(self.codec_params.as_ref(), mode_params, metadata)
    }
}
```

#### 4.3 Example Configuration File Changes (`config/config.yaml`)

```yaml
profiles:
  # ========================================
  # EXISTING X265 PROFILES (unchanged)
  # ========================================

  movie:
    title: "Standard Movie"
    codec: "x265"  # NEW: Explicit codec specification
    base_crf: 22
    bitrate: 10000
    content_type: "film"
    x265_params:
      preset: "slow"
      pix_fmt: "yuv420p10le"
      profile: "main10"
      no-sao: true
      bframes: 5
      b-adapt: 2
      ref: 4
      psy-rd: 1.5
      psy-rdoq: 2.0
      aq-mode: 2
      aq-strength: 0.9
      deblock: "-1,-1"
      rc-lookahead: 40
      merange: 57
      max-tu-size: 32
      ctu: 64
      rd: 4
      rdoq-level: 2
      qcomp: 0.70
      weightb: true
      weightp: true
      cutree: true
      me: "hex"
      subme: 3

  # ========================================
  # NEW: AV1 PROFILES (libaom-av1)
  # ========================================

  movie_av1_aom:
    title: "Standard Movie (AV1 libaom)"
    codec: "av1-aom"
    base_crf: 30  # AV1 optimal range is ~8 points higher than x265
    bitrate: 8000  # AV1 is ~30% more efficient
    content_type: "film"
    av1_params:
      cpu_used: 4           # 0-8, lower = slower/better (4 = medium)
      pix_fmt: "yuv420p10le"
      enable_qm: 1          # Quantization matrices
      qm_min: 0
      qm_max: 15
      aq_mode: 1            # Adaptive quantization
      tune_content: 0       # 0=default, 1=screen content
      enable_cdef: 1        # Constrained directional enhancement filter
      enable_restoration: 1  # Loop restoration filter
      arnr_strength: 4      # Temporal filtering strength
      arnr_maxframes: 7     # Max frames for temporal filtering
      lag_in_frames: 35     # Lookahead frames

  movie_av1_aom_quality:
    title: "High Quality Movie (AV1 libaom)"
    codec: "av1-aom"
    base_crf: 28
    bitrate: 10000
    content_type: "film"
    av1_params:
      cpu_used: 2           # Slower for better quality
      pix_fmt: "yuv420p10le"
      enable_qm: 1
      qm_min: 0
      qm_max: 15
      aq_mode: 1
      tune_content: 0
      enable_cdef: 1
      enable_restoration: 1
      arnr_strength: 5
      arnr_maxframes: 15
      lag_in_frames: 48

  # ========================================
  # NEW: AV1 PROFILES (SVT-AV1)
  # ========================================

  movie_av1_svt:
    title: "Standard Movie (AV1 SVT-AV1)"
    codec: "svt-av1"
    base_crf: 30
    bitrate: 8000
    content_type: "film"
    svtav1_params:
      preset: 6             # 0-13, lower = slower/better (6 = medium)
      pix_fmt: "yuv420p10le"
      tune: 0               # 0=VQ (visual quality), 1=PSNR
      film-grain: 0         # Film grain synthesis
      enable-overlays: 1    # Compound prediction
      scd: 1                # Scene change detection
      lookahead: 60         # Lookahead distance

  movie_av1_svt_fast:
    title: "Fast Movie (AV1 SVT-AV1)"
    codec: "svt-av1"
    base_crf: 32
    bitrate: 7000
    content_type: "film"
    svtav1_params:
      preset: 8             # Faster preset
      pix_fmt: "yuv420p10le"
      tune: 0
      film-grain: 0
      enable-overlays: 1
      scd: 1
      lookahead: 40

  # ========================================
  # AV1 ANIME PROFILES
  # ========================================

  anime_av1:
    title: "Anime (AV1 SVT-AV1 optimized)"
    codec: "svt-av1"
    base_crf: 32
    bitrate: 6000
    content_type: "anime"
    svtav1_params:
      preset: 4             # Slower for anime detail preservation
      pix_fmt: "yuv420p10le"
      tune: 0
      enable-overlays: 1
      enable-qm: 1          # Quantization matrices for flat colors
      qm-min: 0
      film-grain: 0         # Usually not needed for anime
      scd: 1
      lookahead: 80         # Higher lookahead for static scenes

  anime_av1_aom:
    title: "Anime (AV1 libaom)"
    codec: "av1-aom"
    base_crf: 32
    bitrate: 6000
    content_type: "anime"
    av1_params:
      cpu_used: 3
      pix_fmt: "yuv420p10le"
      enable_qm: 1
      qm_min: 0
      qm_max: 15
      aq_mode: 3            # Variance-based AQ for flat regions
      tune_content: 0
      enable_cdef: 1
      enable_restoration: 1
      arnr_strength: 3
      lag_in_frames: 48

  # ========================================
  # AV1 4K PROFILES
  # ========================================

  4k_av1_svt:
    title: "4K Movie (AV1 SVT-AV1)"
    codec: "svt-av1"
    base_crf: 28
    bitrate: 12000
    content_type: "film"
    svtav1_params:
      preset: 5
      pix_fmt: "yuv420p10le"
      tune: 0
      film-grain: 0
      enable-overlays: 1
      scd: 1
      lookahead: 60
      tile-columns: 2       # Parallel encoding for 4K
      tile-rows: 1
```

### Backwards Compatibility Examples

```yaml
# OLD FORMAT (still fully supported)
profiles:
  movie_old:
    title: "Standard Movie (legacy format)"
    base_crf: 22
    bitrate: 10000
    content_type: "film"
    # No codec field = defaults to "x265"
    x265_params:
      preset: "slow"
      # ...

# NEW FORMAT (recommended)
profiles:
  movie_new:
    title: "Standard Movie (new format)"
    codec: "x265"  # Explicit codec
    base_crf: 22
    bitrate: 10000
    content_type: "film"
    x265_params:
      preset: "slow"
      # ...
```

---

## File Structure Changes

### New Files to Create

```
src/encoding/codec/
├── mod.rs                 # Codec trait definitions, exports
├── factory.rs             # Codec factory implementation
├── x265.rs                # X265Codec + X265Params implementation
├── av1_aom.rs             # Av1AomCodec + Av1AomParams implementation
├── svt_av1.rs             # SvtAv1Codec + SvtAv1Params implementation
└── traits.rs              # Additional codec-related traits (optional)

src/hdr/
└── codec_adapter.rs       # HDR translation layer per codec
```

### Files to Modify

```
src/config/
├── types.rs               # Add codec field to RawProfile
├── profiles.rs            # Update EncodingProfile with codec support
└── loader.rs              # Add codec validation during load

src/encoding/
├── mod.rs                 # Export codec module
├── modes.rs               # Replace hardcoded libx265 with factory
└── options.rs             # Add codec option to CLI (optional)

src/hdr/
├── encoding.rs            # Refactor to use HdrCodecAdapter
└── metadata.rs            # Add codec-agnostic formatters

src/cli/
└── args.rs                # Update help text to mention AV1 support

src/main.rs                # Initialize codec factory in main
```

---

## Testing Strategy

### Unit Tests

#### 1. Codec Factory Tests (`src/encoding/codec/factory.rs`)

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_codec_registration() {
        let factory = CodecFactory::new();
        assert!(factory.get_codec("x265").is_ok());
        assert!(factory.get_codec("av1-aom").is_ok());
        assert!(factory.get_codec("svt-av1").is_ok());
    }

    #[test]
    fn test_missing_codec() {
        let factory = CodecFactory::new();
        assert!(factory.get_codec("nonexistent").is_err());
    }

    #[test]
    fn test_list_codecs() {
        let factory = CodecFactory::new();
        let codecs = factory.list_codecs();
        assert!(codecs.contains(&"x265".to_string()));
        assert!(codecs.contains(&"av1-aom".to_string()));
        assert!(codecs.contains(&"svt-av1".to_string()));
    }
}
```

#### 2. Parameter Translation Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_x265_params_unchanged() {
        // Ensure x265 parameter building produces identical output
        let params = X265Params::new();
        // ... verify output matches legacy implementation
    }

    #[test]
    fn test_av1_hdr_metadata() {
        let mut params = Av1AomParams::new("4".to_string());
        let hdr_metadata = HdrMetadata::hdr10_default();
        params.with_hdr_metadata(&hdr_metadata);

        // Verify HDR params are correctly formatted for AV1
        assert!(params.params.contains_key("color-primaries"));
        assert_eq!(params.params.get("color-primaries"), Some(&"bt2020".to_string()));
    }
}
```

#### 3. Profile Loading Tests

```rust
#[cfg(test)]
mod tests {
    #[test]
    fn test_backwards_compatible_profile() {
        // Load old-format profile (no codec field)
        let yaml = r#"
        title: "Test"
        base_crf: 22
        bitrate: 10000
        content_type: "film"
        x265_params:
          preset: "slow"
        "#;

        let raw: RawProfile = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(raw.codec, "x265"); // Should default to x265
    }

    #[test]
    fn test_new_av1_profile() {
        let yaml = r#"
        title: "Test AV1"
        codec: "av1-aom"
        base_crf: 30
        bitrate: 8000
        content_type: "film"
        av1_params:
          cpu_used: 4
        "#;

        let raw: RawProfile = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(raw.codec, "av1-aom");
    }
}
```

### Integration Tests

#### 1. Encoding Tests

```rust
#[tokio::test]
async fn test_x265_encoding_unchanged() {
    // Ensure x265 encoding produces same output as before refactor
    // Compare output files, encoding logs, etc.
}

#[tokio::test]
async fn test_av1_encoding_works() {
    // Test AV1 encoding with all modes (CRF, ABR, CBR)
}

#[tokio::test]
async fn test_hdr_passthrough_per_codec() {
    // Test HDR metadata preservation for x265 and AV1
}
```

#### 2. Configuration Tests

```rust
#[test]
fn test_config_loading_backwards_compatible() {
    // Load old config file, verify profiles work
}

#[test]
fn test_new_config_with_av1() {
    // Load new config with AV1 profiles, verify correct codec selected
}

#[test]
fn test_invalid_codec_handling() {
    // Test graceful error handling for invalid codec names
}
```

---

## Key Advantages

### 1. **Zero Breaking Changes**
- All existing x265 functionality preserved byte-for-byte
- Existing configuration files work without modification
- Default codec is x265 for backwards compatibility
- Legacy `x265_params` field still supported

### 2. **Clean Separation of Concerns**
- Codec logic isolated in `src/encoding/codec/` module
- HDR metadata translation abstracted via adapter pattern
- Profile configuration cleanly separated from codec implementation
- Easy to add new codecs without touching existing code

### 3. **Type Safety**
- Trait-based design ensures compile-time checks
- Factory pattern prevents invalid codec access
- Clear interfaces between layers
- No runtime string parsing where avoidable

### 4. **Maintainability**
- Codec-specific code in dedicated files (x265.rs, av1_aom.rs, etc.)
- Easy to update individual codec implementations
- Clear testing boundaries per codec
- Self-documenting code structure

### 5. **Extensibility**
- Adding VP9, H.264, or future codecs is straightforward
- HDR adapter pattern scales to new HDR formats
- Configuration schema supports arbitrary codecs
- No modifications needed to core encoding logic

### 6. **Performance**
- Factory pattern with HashMap lookup (O(1) codec retrieval)
- Trait objects enable efficient dynamic dispatch
- No performance regression for existing x265 usage
- Lazy codec initialization possible

### 7. **Testing & Validation**
- Unit tests per codec ensure correctness
- Integration tests verify backwards compatibility
- Clear boundaries make mocking/stubbing easy
- Configuration validation at load time

---

## Codec Parameter Mapping Reference

### X265 → AV1 Parameter Equivalents

| x265 Parameter | AV1 (libaom) Equivalent | SVT-AV1 Equivalent | Notes |
|----------------|-------------------------|---------------------|-------|
| `preset` | `cpu-used` (0-8) | `preset` (0-13) | Different scales; lower = slower/better |
| `crf` | `crf` (0-63) | `crf` (0-63) | AV1 range is wider, optimal values differ |
| `aq-mode` | `aq-mode` (0-3) | `enable-qm` (0-1) | AV1 has different AQ implementation |
| `psy-rd` | `tune-content` | `tune` (0-1) | Different psychovisual tuning approach |
| `deblock` | `enable-cdef` | `enable-cdef` | AV1 uses CDEF (Constrained Directional Enhancement Filter) |
| `sao` | `enable-restoration` | `enable-restoration` | AV1 loop restoration filter |
| `bframes` | N/A | N/A | AV1 doesn't use traditional B-frames |
| `ref` | `ref-frames` | `ref-frames` | Similar concept, different implementation |
| `colorprim=bt2020` | `color-primaries=bt2020` | Same | Slightly different parameter naming |
| `transfer=smpte2084` | `transfer-characteristics=smpte2084` | Same | Longer parameter name for AV1 |
| `colormatrix=bt2020nc` | `matrix-coefficients=bt2020nc` | Same | Different parameter naming |
| `master-display` | `mastering-display` | Same | Different format syntax |
| `max-cll` | `content-light-level` | Same | Different parameter naming |

### Optimal CRF Ranges by Codec

| Codec | CRF Range | Optimal for Most Content | Notes |
|-------|-----------|--------------------------|-------|
| **x265** | 0-51 | 18-28 (22-24 typical) | Industry standard for HEVC |
| **AV1 (libaom)** | 0-63 | 25-35 (30-32 typical) | ~8 points higher than x265 |
| **AV1 (SVT-AV1)** | 0-63 | 28-38 (32-35 typical) | ~10 points higher than x265 |

**Why AV1 CRF is higher:** AV1's CRF scale is calibrated differently. CRF 30 in AV1 is roughly equivalent to CRF 22 in x265 in terms of perceived quality.

### Preset Performance Comparison

| Quality Target | x265 Preset | libaom-av1 | SVT-AV1 | Relative Speed |
|----------------|-------------|------------|---------|----------------|
| **Fast** | fast | cpu-used=6 | preset=8 | AV1 is 2-3x slower |
| **Balanced** | medium | cpu-used=4 | preset=6 | AV1 is 3-5x slower |
| **Quality** | slow | cpu-used=2 | preset=4 | AV1 is 5-10x slower |
| **Archive** | veryslow | cpu-used=0 | preset=2 | AV1 is 10-20x slower |

**Note:** SVT-AV1 is generally 2-3x faster than libaom-av1 at similar quality levels.

### HDR Parameter Differences

#### X265 HDR Parameters
```
-x265-params colorprim=bt2020:transfer=smpte2084:colormatrix=bt2020nc:master-display=G(x,y)B(x,y)R(x,y)WP(x,y)L(max,min):max-cll=max,fall
```

#### AV1 (libaom-av1) HDR Parameters
```
-color-primaries bt2020 -transfer-characteristics smpte2084 -matrix-coefficients bt2020nc -mastering-display "Gx,Gy,Bx,By,Rx,Ry,WPx,WPy,Lmax,Lmin" -content-light-level "max,fall"
```

#### SVT-AV1 HDR Parameters
```
-svtav1-params color-primaries=bt2020:transfer-characteristics=smpte2084:matrix-coefficients=bt2020nc:mastering-display=Gx:Gy:Bx:By:Rx:Ry:WPx:WPy:Lmax:Lmin:content-light-level=max:fall
```

### Bitrate Efficiency Comparison

| Codec | Bitrate for Same Quality | File Size Reduction | Notes |
|-------|---------------------------|---------------------|-------|
| **x265** | 100% (baseline) | 0% | Reference point |
| **AV1 (libaom)** | ~70% | ~30% smaller | At equivalent quality |
| **AV1 (SVT-AV1)** | ~75% | ~25% smaller | Slightly less efficient than libaom |

**Example:** If x265 needs 10 Mbps for a certain quality level, AV1 needs only ~7 Mbps for equivalent quality.

---

## Summary

This modular AV1 integration plan provides:

1. **Complete backwards compatibility** - Existing x265 workflows remain unchanged
2. **Clean codec abstraction** - Trait-based design with factory pattern
3. **Minimal configuration changes** - Simple `codec` field addition
4. **Extensible architecture** - Easy to add VP9, H.264, or future codecs
5. **HDR support per codec** - Proper metadata translation for each encoder
6. **Type-safe implementation** - Compile-time guarantees for codec operations
7. **Clear testing strategy** - Unit and integration tests ensure correctness

### Implementation Sequence

1. **Phase 1**: Create codec abstraction layer (traits, factory)
2. **Phase 2**: Refactor x265 into modular codec implementation
3. **Phase 3**: Implement AV1 codec support (libaom-av1, SVT-AV1)
4. **Phase 4**: Update configuration structures
5. **Phase 5**: Modify encoding modes to use codec factory
6. **Phase 6**: Add HDR parameter translation layer
7. **Phase 7**: Comprehensive testing and validation

### Migration Path for Users

1. **Immediate**: Existing configs work without changes
2. **Recommended**: Add `codec: "x265"` to profiles explicitly
3. **Optional**: Create new AV1 profiles using provided examples
4. **Future**: Use `--migrate-config` tool to auto-convert configs

This plan ensures a smooth transition to multi-codec support while maintaining the high code quality and functionality of the existing codebase.
