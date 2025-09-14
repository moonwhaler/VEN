<img src="https://r2cdn.perplexity.ai/pplx-full-logo-primary-dark%402x.png" style="height:64px;margin-right:32px"/>

# Dolby Vision Preservation: Your Implementation Will NOT Preserve Dolby Vision

Based on your log output and the parameters your Rust implementation is using, **your encoding process will NOT preserve Dolby Vision metadata**. Here's why and what's happening:

## What Your Implementation Is Doing

Your implementation is correctly detecting HDR content and applying HDR10 parameters:

- **Color Space**: bt2020nc[log]
- **Transfer Function**: smpte2084[log]
- **Color Primaries**: bt2020[log]
- **HDR10 metadata**: max-cll and master-display values[log]

However, these are **HDR10 parameters, not Dolby Vision parameters**. Your implementation is essentially converting the Dolby Vision content to HDR10.

## The Fundamental Problem

**Dolby Vision requires RPU (Reference Processing Unit) metadata** that contains frame-by-frame dynamic metadata. This RPU data is what makes Dolby Vision superior to HDR10's static metadata. When you re-encode with standard x265 parameters without preserving the RPU, **the Dolby Vision metadata is completely lost**.[^1][^2][^3][^4][^5]

## What Happens During Standard x265 Encoding

When ffmpeg/x265 processes a Dolby Vision file without special RPU handling:

1. **The RPU metadata is stripped away**[^3][^6]
2. **Only the base layer (BL) HDR10 content remains**[^7][^1]
3. **The dynamic per-frame metadata is lost**[^5][^8]
4. **The result is HDR10 content, not Dolby Vision**[^3]

## Technical Requirements for Dolby Vision Preservation

To preserve Dolby Vision, your implementation would need to:

### 1. Extract RPU Metadata First

```bash
ffmpeg -i input.mkv -c:v copy -vbsf hevc_mp4toannexb -f hevc - | dovi_tool extract-rpu - -o video.rpu
```


### 2. Use x265 with Dolby Vision Parameters

```bash
x265 --dolby-vision-rpu video.rpu --dolby-vision-profile 8.1 --vbv-bufsize 20000 --vbv-maxrate 20000
```


### 3. Inject RPU Back After Encoding

```bash
dovi_tool inject-rpu -i encoded.hevc --rpu-in video.rpu -o final.hevc
```


## Profile Conversion Implications

Most re-encoding workflows convert **Profile 7 (dual-layer) to Profile 8.1 (single-layer)**:[^9][^1][^7]

- **Profile 7**: Dual-layer with Base Layer + Enhancement Layer + RPU (original Blu-ray format)[^9]
- **Profile 8.1**: Single-layer with HDR10 compatibility + RPU[^10][^9]
- **Enhancement Layer (EL) is typically discarded** during conversion[^11][^7]


## Current Dolby Vision Support in x265

x265 **does support Dolby Vision encoding** but only for specific profiles:[^12][^10]

- **Profile 5**: Single-layer Dolby Vision only
- **Profile 8.1**: Single-layer with HDR10 compatibility
- **Profile 8.2**: Single-layer with SDR compatibility

However, **x265 cannot preserve dual-layer Profile 7** directly.[^13][^10]

## FFmpeg Dolby Vision Support Status

Recent FFmpeg versions (5.0+) **can preserve Dolby Vision when remuxing** with `-c:v copy`, but **standard re-encoding with libx265 strips the metadata** unless specific RPU handling is implemented.[^6][^14][^5][^3]

## Recommendations for Your Rust Implementation

To add Dolby Vision support, you would need to:

1. **Detect Dolby Vision content** (check for RPU presence)
2. **Extract RPU metadata** using tools like `dovi_tool`
3. **Use x265 with `--dolby-vision-rpu` and `--dolby-vision-profile`** parameters[^15][^12]
4. **Handle profile conversion** (likely 7 → 8.1)
5. **Implement proper VBV settings** required for Dolby Vision[^3]

## Current Workflow Impact

Your current implementation will:

- ✅ **Preserve HDR10 quality and metadata**
- ✅ **Maintain wide color gamut and high dynamic range**
- ❌ **Lose Dolby Vision's dynamic metadata**
- ❌ **Convert DV content to static HDR10**

The resulting files will still look excellent on HDR10-compatible displays, but **Dolby Vision displays will not show the "Dolby Vision" indicator** and will fall back to HDR10 mode, losing the superior dynamic metadata that makes Dolby Vision special.[^2][^16]
<span style="display:none">[^17][^18][^19][^20][^21][^22][^23][^24][^25][^26][^27][^28][^29][^30][^31][^32][^33][^34][^35][^36][^37][^38][^39][^40][^41][^42][^43][^44][^45]</span>

<div style="text-align: center">⁂</div>

[^1]: https://forum.makemkv.com/forum/viewtopic.php?t=26514

[^2]: https://eureka.patsnap.com/blog/dolby-vision-vs-hdr10/

[^3]: https://codecalamity.com/encoding-uhd-4k-hdr10-videos-with-ffmpeg/

[^4]: https://www.reliant.co.uk/blog/hdr10-vs-dolby-vision-whats-the-difference/

[^5]: https://www.reddit.com/r/ffmpeg/comments/1dupvue/lots_of_questions_regarding_hdr_and_especially/

[^6]: https://trac.ffmpeg.org/ticket/7037

[^7]: https://github.com/quietvoid/dovi_tool/discussions/195

[^8]: https://www.reddit.com/r/ffmpeg/comments/15z1gzd/question_for_reencoding_hevc_video_with_dolby/

[^9]: https://www.reddit.com/r/Dolby/comments/1kg2rhe/can_someone_explain_the_differences_between_dolby/

[^10]: https://forum.doom9.org/showthread.php?t=181868

[^11]: https://www.reddit.com/r/ffmpeg/comments/11gu4o4/convert_dv_profile_7_to_81_using_dovi_tool_mp4box/

[^12]: https://x265.readthedocs.io/en/master/cli.html

[^13]: https://forum.doom9.org/showthread.php?p=1937112

[^14]: https://trac.ffmpeg.org/ticket/9131

[^15]: https://x265.readthedocs.io/en/3.1/releasenotes.html

[^16]: https://www.displayninja.com/hdr10-vs-dolby-vision/

[^17]: https://www.reddit.com/r/handbrake/comments/1hjxyhp/encoding_with_dolby_vision_vs_injecting_rpu/

[^18]: https://forum.doom9.org/showthread.php?p=2015260

[^19]: https://github.com/HandBrake/HandBrake/issues/4144

[^20]: https://professionalsupport.dolby.com/s/article/Dolby-Vision-Encoding-of-mezzanine-assets

[^21]: https://www.reddit.com/r/PleX/comments/1jprfzs/preserving_dolby_vision_metadata_with_the_new/

[^22]: https://forum.blackmagicdesign.com/viewtopic.php?t=216263\&p=1124088

[^23]: https://www.strong-eu.com/blog/hdr-hdr10-hdr10-and-dolby-vision-what-are-the-differences

[^24]: https://codecalamity.com/encoding-settings-for-hdr-4k-videos-using-10-bit-x265/

[^25]: https://github.com/HandBrake/HandBrake/issues/5813

[^26]: https://www.dolby.com/experience/home-entertainment/articles/the-difference-between-hdr10-and-dolby-vision/

[^27]: https://stackoverflow.com/questions/70572976/integrate-dolby-vision-8-4-metadata-into-streams-encoded-by-x265

[^28]: https://news.ycombinator.com/item?id=32946427

[^29]: https://www.reddit.com/r/handbrake/comments/ndfvee/hdr10_vs_10_vs_12_dolby_vision_for_265_reencoding/

[^30]: https://forum.doom9.net/showthread.php?t=183479\&page=44

[^31]: https://github.com/rigaya/NVEnc/issues/663

[^32]: https://forum.doom9.org/showthread.php?t=183292

[^33]: https://forum.doom9.org/showthread.php?t=176006\&page=2

[^34]: https://www.reddit.com/r/PleX/comments/1jgsadm/dolby_vision_profile_8_vs_7_noticeable_difference/

[^35]: https://community.firecore.com/t/dolby-vision-profile-7-8-support-ts-mkv-files/19713/1658

[^36]: https://professionalsupport.dolby.com/s/article/Dolby-Vision-Encoding-using-Blackmagic-Design-DaVinci-Resolve-Studio-AQs

[^37]: https://github.com/HandBrake/HandBrake/issues/6458

[^38]: https://forum.blackmagicdesign.com/viewtopic.php?f=21\&t=150538

[^39]: https://community.firecore.com/t/dolby-vision-profile-7-8-support-ts-mkv-files/19713?page=44

[^40]: https://forum.doom9.org/showthread.php?p=1954660

[^41]: https://emby.media/community/index.php?%2Ftopic%2F138363-converting-dv-profile-76-to-a-playable-profile%2F

[^42]: https://discourse.coreelec.org/t/learning-about-dolby-vision-and-coreelec-development/50998?page=32

[^43]: https://github.com/quietvoid/dovi_tool/issues/44

[^44]: https://professionalsupport.dolby.com/s/article/Transcoding-Dolby-Vision-profile-8-4-to-SDR-on-Android

[^45]: https://x265.readthedocs.io/en/stable/cli.html

