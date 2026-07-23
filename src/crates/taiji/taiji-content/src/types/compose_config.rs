use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// FFmpeg compose config: frame input + audio input + subtitles + encoding parameters.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComposeConfig {
    /// PNG frame sequence directory (intermediate product, independently managed)
    pub frames_dir: PathBuf,
    /// Frame filename pattern, e.g. "frame_%04d.png"
    pub frame_pattern: String,
    /// MP3 audio file path (intermediate product)
    pub audio_path: PathBuf,
    /// SRT subtitle file path (optional)
    pub subtitle_path: Option<PathBuf>,
    /// Output MP4 path
    pub output_path: PathBuf,
    /// Encoding profile
    pub encoding: EncodingProfile,
}

/// Encoding profile.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncodingProfile {
    /// Video codec, e.g. "libx264" | "h264_amf" | "h264_qsv" | "h264_nvenc"
    pub codec: String,
    /// Encoding preset, e.g. "medium" | "fast" | "ultrafast"
    pub preset: String,
    /// CRF quality (0-51, 18 is visually lossless, 23 is recommended)
    pub crf: u8,
    /// Frame rate
    pub fps: u8,
    /// Output resolution (width, height)
    pub resolution: (u16, u16),
}

impl Default for EncodingProfile {
    fn default() -> Self {
        Self {
            codec: "libx264".into(),
            preset: "medium".into(),
            crf: 23,
            fps: 30,
            resolution: (1920, 1080),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encoding_profile_default() {
        let profile = EncodingProfile::default();
        assert_eq!(profile.codec, "libx264");
        assert_eq!(profile.resolution, (1920, 1080));
        assert_eq!(profile.crf, 23);
    }

    #[test]
    fn test_compose_config_roundtrip() {
        let config = ComposeConfig {
            frames_dir: PathBuf::from("output/frames"),
            frame_pattern: "frame_%04d.png".into(),
            audio_path: PathBuf::from("output/narration.mp3"),
            subtitle_path: Some(PathBuf::from("output/narration.srt")),
            output_path: PathBuf::from("output/final.mp4"),
            encoding: EncodingProfile::default(),
        };
        let json = serde_json::to_string(&config).unwrap();
        let roundtrip: ComposeConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(roundtrip.frame_pattern, "frame_%04d.png");
        assert_eq!(roundtrip.encoding.fps, 30);
        assert!(roundtrip.subtitle_path.is_some());
    }

    #[test]
    fn test_compose_config_no_subtitle() {
        let config = ComposeConfig {
            frames_dir: PathBuf::from("output/frames"),
            frame_pattern: "frame_%04d.png".into(),
            audio_path: PathBuf::from("output/narration.mp3"),
            subtitle_path: None,
            output_path: PathBuf::from("output/final.mp4"),
            encoding: EncodingProfile::default(),
        };
        let json = serde_json::to_string(&config).unwrap();
        let roundtrip: ComposeConfig = serde_json::from_str(&json).unwrap();
        assert!(roundtrip.subtitle_path.is_none());
    }
}
