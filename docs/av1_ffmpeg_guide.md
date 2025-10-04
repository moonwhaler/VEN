# FFmpeg AV1 Encoding Guide

**Main Takeaway:** For highest quality at reasonable speed, use SVT-AV1 with tuned parameters for different content types. For ultimate archival quality, use libaom-av1. Hardware-accelerated encodes (QSV/Va-API/NVENC) trade quality for speed.

---

## 1. Encoders Overview

- **libaom-av1**: Reference encoder with best compression efficiency. Very slow; ideal for *archival* encodes.  
- **SVT-AV1**: Intel/Netflix scalable encoder. Excellent *speed-to-quality* balance. Recommended for large batches.  
- **rav1e**: Rust-based encoder focusing on simplicity and safety. Faster than libaom but lower efficiency.  
- **Hardware (av1_qsv / av1_nvenc)**: Fastest but lower quality. Suitable for realtime or live streaming.

---

## 2. Common Parameters

All commands assume input `input.mkv`. Adjust thread count (`-threads`), bitrate (`-b:v`), and CRF (`-crf`) per content:

- `-c:v <encoder>`  
- `-crf <value>` (libaom/SVT/rav1e)  
- `-b:v <bitrate>` (CBR modes)  
- `-preset <0–8>` / `-cpu-used <0–8>`  
- `-tiles <columns>x<rows>` (multi-threading)  
- `-row-mt 1 -frame-parallel 1` (increases threading)  
- `-g <keyint>` (GOP size)  
- `-pix_fmt yuv420p10le` (10-bit)

---

## 3. Anime Encoding

Anime benefits from sharper edges, dark scenes, and flat areas:

```bash
ffmpeg -i input.mkv \
  -c:v libsvtav1 \
  -preset 6 \
  -crf 24 \
  -g 240 \
  -tiles 2x2 \
  -row-mt 1 \
  -aq-mode 3 \
  -aq-strength 0.8 \
  -pix_fmt yuv420p10le \
  -c:a copy \
  output_anime_av1.mkv
```
- **AQ mode 3** boosts dark scene quality (prevents banding) and is critical for anime.  
- **Preset 6** balances speed and compression.  
- **10-bit** improves dynamic range and banding.

---

## 4. Classic Movies (Dialogue-Driven)

Balanced settings focusing on grain preservation and consistent quality:

```bash
ffmpeg -i input.mkv \
  -c:v libaom-av1 \
  -crf 30 \
  -cpu-used 2 \
  -g 300 \
  -tiles 4x2 \
  -row-mt 1 \
  -pix_fmt yuv420p10le \
  -aq-mode 2 \
  -aq-strength 1 \
  -c:a libopus -b:a 128k \
  output_classic_av1.mkv
```
- **CRF 30** for smaller file size with acceptable quality.  
- **AQ mode 2** suits varied textures.  
- **Tiles 4×2** speeds up encode without large quality loss.  

---

## 5. Modern Action Movies

High motion scenes need lower CRF and faster presets:

```bash
ffmpeg -i input.mkv \
  -c:v libsvtav1 \
  -preset 5 \
  -crf 22 \
  -g 120 \
  -tiles 4x3 \
  -row-mt 1 \
  -frame-parallel 1 \
  -pix_fmt yuv420p10le \
  -aq-mode 2 \
  -aq-strength 1 \
  -c:a libopus -b:a 192k \
  output_action_av1.mkv
```
- **CRF 22** retains detail in fast motion.  
- **Preset 5** accelerates encode while maintaining quality.  
- **GOP 120** aids scene change handling.

---

## 6. Hardware-Accelerated Encoding

### Intel QSV (Arc & Integrated GPUs)
```bash
ffmpeg -init_hw_device vaapi=va:/dev/dri/renderD128 \
  -i input.mkv \
  -c:v av1_qsv \
  -preset veryslow \
  -look_ahead_depth 40 \
  -extra_hw_frames 40 \
  -b:v 2M \
  -bufsize 4M \
  -rc_init_occupancy 1M \
  output_hw_av1.mp4
```
- **Look-ahead & extra frames** improve rate control.  
- **Bufsize ≥2×bitrate** recommended for CBR.

### NVIDIA NVENC AV1 (40-series)
```bash
ffmpeg -i input.mkv \
  -c:v av1_nvenc \
  -preset p5 \
  -rc:v CBR \
  -b:v 2M \
  -g 240 \
  -pix_fmt yuv420p10le \
  output_nvenc_av1.mkv
```
- Lower visual quality vs CPU encoders but ~10× faster.

---

## 7. Expert Tips & Resources

- **SVT-AV1 Tune**: `tune=0` (SSIM) preferred over subjective SSIM or PSY stock for fewer artifacts.  
- **Tile configuration**: Minimize tiles for compression efficiency; increase only as needed for threading.  
- **Lossless sources**: Always start from highest-quality source (lossless remux or original master).  
- **Software tools**: Consider GUI helpers like `av1an` or `shutter-encoder` for pipelines.

### Key Forum Links
- Doom9 AV1 discussion: https://forum.doom9.org/showthread.php?t=185524  
- r/AV1 “Anime Library” thread: https://www.reddit.com/r/AV1/comments/wcpny5/…  
- r/AV1 ffmpeg tips: https://www.reddit.com/r/AV1/comments/17vf69p/…  
- Level1Techs GPU AV1 QSV: https://forum.level1techs.com/t/ffmpeg-av1-encoding-using-intel-arc-gpu-tips/205120  
- Streaming Media AV1 performance: https://www.streamingmedia.com/Articles/ReadArticle.aspx?ArticleID=130284  

*This document is provided in Markdown for easy integration into repositories or personal wikis.*