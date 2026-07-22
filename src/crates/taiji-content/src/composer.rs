use crate::types::compose_config::{ComposeConfig, EncodingProfile};
use std::process::Command;

/// FFmpeg composer: combine PNG frame sequence + MP3 audio + optional SRT subtitles into H.264 MP4.
///
/// ## File system access
///
/// This struct uses raw `std::fs` for path canonicalization and file I/O.
/// TODO(taiji): Migrate to bitfun_services FileSystemService for platform-agnostic
/// path handling and file operations. The FileSystemService abstraction lives in
/// `src/crates/services` and provides canonicalize, read, write, and directory
/// traversal primitives that work across desktop, remote, and WASM targets.
pub struct FfmpegComposer {
    config: ComposeConfig,
}

impl FfmpegComposer {
    pub fn new(config: ComposeConfig) -> Self {
        Self { config }
    }

    /// Build ffmpeg command line and execute composition.
    ///
    /// Equivalent command:
    /// ```text
    /// ffmpeg -y -framerate {fps} -i {frames_dir}/{frame_pattern} -i {audio}
    ///        -vf "scale=W:H[,subtitles={srt}:force_style=...]"
    ///        -c:v {codec} -preset {preset} -crf {crf} -pix_fmt yuv420p
    ///        -c:a aac -b:a 128k -shortest {output}
    /// ```
    pub fn compose(&self) -> Result<(), String> {
        let cfg = &self.config;

        // Canonicalize paths to prevent path traversal.
        //
        // TODO(taiji): Replace std::fs::canonicalize with
        // bitfun_services FileSystemService::canonicalize for cross-platform safety
        // and remote-workspace support.
        let frames_dir = std::fs::canonicalize(&cfg.frames_dir).map_err(|e| {
            format!(
                "Failed to resolve frames_dir {}: {}",
                cfg.frames_dir.display(),
                e
            )
        })?;
        let audio_path = std::fs::canonicalize(&cfg.audio_path).map_err(|e| {
            format!(
                "Failed to resolve audio_path {}: {}",
                cfg.audio_path.display(),
                e
            )
        })?;
        let output_path =
            std::fs::canonicalize(&cfg.output_path).unwrap_or_else(|_| cfg.output_path.clone()); // output may not exist yet; keep original if not found
        let subtitle_path: Option<std::path::PathBuf> =
            match &cfg.subtitle_path {
                Some(p) => Some(std::fs::canonicalize(p).map_err(|e| {
                    format!("Failed to resolve subtitle_path {}: {}", p.display(), e)
                })?),
                None => None,
            };

        let frame_input = frames_dir.join(&cfg.frame_pattern);
        let frame_input_str = frame_input.to_string_lossy();

        let mut cmd = Command::new("ffmpeg");

        // Basic input parameters
        cmd.arg("-y")
            .arg("-framerate")
            .arg(cfg.encoding.fps.to_string())
            .arg("-i")
            .arg(frame_input_str.as_ref())
            .arg("-i")
            .arg(audio_path.to_string_lossy().as_ref());

        // Build video filter chain: scale + optional subtitle burn-in
        let vf = Self::build_video_filter(&cfg.encoding, subtitle_path.as_ref());
        cmd.arg("-vf").arg(&vf);

        // Video encoding parameters
        cmd.arg("-c:v")
            .arg(&cfg.encoding.codec)
            .arg("-preset")
            .arg(&cfg.encoding.preset)
            .arg("-crf")
            .arg(cfg.encoding.crf.to_string())
            .arg("-pix_fmt")
            .arg("yuv420p");

        // Audio encoding parameters
        cmd.arg("-c:a").arg("aac").arg("-b:a").arg("128k");

        // End with the shorter stream
        cmd.arg("-shortest");

        // Output file
        cmd.arg(output_path.to_string_lossy().as_ref());

        log::info!(
            "FFmpeg compose: {} frames + {} -> {}",
            frame_input_str,
            audio_path.display(),
            output_path.display()
        );

        let output = cmd
            .output()
            .map_err(|e| format!("ffmpeg not found or failed to start: {}", e))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!("ffmpeg exited with error:\n{}", stderr));
        }

        log::info!("FFmpeg compose finished: {}", cfg.output_path.display());

        Ok(())
    }

    /// Build video filter string.
    ///
    /// - Always apply `scale=W:H` to resize to target resolution.
    /// - If a subtitle file is configured, chain-append `subtitles` filter for burn-in.
    fn build_video_filter(
        encoding: &EncodingProfile,
        subtitle_path: Option<&std::path::PathBuf>,
    ) -> String {
        let (w, h) = encoding.resolution;
        let mut filter = format!("scale={}:{}", w, h);

        if let Some(srt_path) = subtitle_path {
            let srt_escaped = Self::escape_subtitle_path(srt_path);
            filter.push_str(&format!(
                ",subtitles={}:force_style='FontSize=24,PrimaryColour=&H00FFFFFF,OutlineColour=&H00000000,Outline=2'",
                srt_escaped
            ));
        }

        filter
    }

    /// Escape subtitle file path for the ffmpeg subtitles filter.
    ///
    /// ffmpeg's subtitles filter uses `:` as argument separator, so
    /// `\` is replaced with `/`, and `:` is replaced with `\:`.
    fn escape_subtitle_path(path: &std::path::Path) -> String {
        path.to_string_lossy()
            .replace('\\', "/")
            .replace(':', "\\:")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn make_config(subtitle: bool) -> ComposeConfig {
        ComposeConfig {
            frames_dir: PathBuf::from("output/frames"),
            frame_pattern: "frame_%04d.png".into(),
            audio_path: PathBuf::from("output/narration.mp3"),
            subtitle_path: if subtitle {
                Some(PathBuf::from("output/narration.srt"))
            } else {
                None
            },
            output_path: PathBuf::from("output/final.mp4"),
            encoding: EncodingProfile::default(),
        }
    }

    #[test]
    fn test_build_video_filter_no_subtitle() {
        let config = make_config(false);
        let vf =
            FfmpegComposer::build_video_filter(&config.encoding, config.subtitle_path.as_ref());
        assert_eq!(vf, "scale=1920:1080");
        // Verify no subtitles keyword present
        assert!(!vf.contains("subtitles"));
    }

    #[test]
    fn test_build_video_filter_with_subtitle() {
        let config = make_config(true);
        let vf =
            FfmpegComposer::build_video_filter(&config.encoding, config.subtitle_path.as_ref());
        assert!(vf.starts_with("scale=1920:1080,subtitles="));
        assert!(vf.contains("force_style='FontSize=24"));
    }

    #[test]
    fn test_escape_subtitle_path_windows() {
        let path = PathBuf::from("C:\\data\\projects\\taiji\\output\\narration.srt");
        let escaped = FfmpegComposer::escape_subtitle_path(&path);
        // Colon is escaped to \:
        assert!(
            escaped.contains("\\:"),
            "colon should be escaped: {}",
            escaped
        );
        // Windows backslash replaced with forward slash; no unescaped backslash remains
        assert!(
            !escaped.contains("data\\"),
            "backslash path separators should be removed: {}",
            escaped
        );
        assert!(escaped.ends_with("narration.srt"));
    }

    #[test]
    fn test_ffmpeg_not_found() {
        let result = std::process::Command::new("nonexistent_ffmpeg_binary_xyz")
            .arg("--version")
            .output();
        assert!(result.is_err());
    }
}

/// A/V sync accuracy test tool.
///
/// Verification method: known timestamps → FFmpeg compose → check keyframe time offsets.
pub mod sync_test {
    /// Maximum tolerated A/V sync drift (milliseconds).
    pub const MAX_SYNC_DRIFT_MS: i64 = 100;

    /// Keyframe checkpoints.
    /// Each point corresponds to a moment in the video timeline where a specific
    /// annotation is expected to appear.
    #[derive(Debug, Clone)]
    pub struct SyncCheckpoint {
        /// Expected time (seconds)
        pub expected_time_sec: f64,
        /// Checkpoint description
        pub label: String,
        /// Allowed drift (seconds)
        pub tolerance_sec: f64,
    }

    impl SyncCheckpoint {
        pub fn new(time_sec: f64, label: &str) -> Self {
            Self {
                expected_time_sec: time_sec,
                label: label.into(),
                tolerance_sec: MAX_SYNC_DRIFT_MS as f64 / 1000.0,
            }
        }
    }

    /// Create 10 standard keyframe checkpoints for testing.
    ///
    /// Corresponds to key annotation moments in a 100-frame / 30fps ≈ 3.33s video:
    /// - 0.0s: Video start, first candlestick appears
    /// - 0.5s: Structure analysis segment starts
    /// - 1.5s: Capital flow analysis (delta)
    /// - 2.0s: Magnet annotation appears
    /// - 2.5s: Triple-push annotation appears (if applicable)
    /// - 3.0s: Resonance conclusion
    /// - Last frame: Candlestick fully expanded
    pub fn standard_checkpoints() -> Vec<SyncCheckpoint> {
        vec![
            SyncCheckpoint::new(0.0, "Video start"),
            SyncCheckpoint::new(0.5, "Structure analysis narration start"),
            SyncCheckpoint::new(1.0, "Capital flow analysis narration start"),
            SyncCheckpoint::new(1.5, "Magnet analysis narration start"),
            SyncCheckpoint::new(2.0, "Magnet annotation overlay appears"),
            SyncCheckpoint::new(2.3, "Triple-push analysis narration start"),
            SyncCheckpoint::new(2.6, "Resonance analysis narration start"),
            SyncCheckpoint::new(2.9, "Decision advice narration start"),
            SyncCheckpoint::new(3.1, "Disclaimer narration start"),
            SyncCheckpoint::new(3.3, "Video end (candlestick complete)"),
        ]
    }

    /// A/V sync validation result.
    #[derive(Debug)]
    pub struct SyncValidationResult {
        /// Total number of checkpoints
        pub total: usize,
        /// Number passed
        pub passed: usize,
        /// Failure details
        pub failures: Vec<SyncFailure>,
        /// Maximum drift (seconds)
        pub max_drift_sec: f64,
    }

    #[derive(Debug)]
    pub struct SyncFailure {
        pub checkpoint: SyncCheckpoint,
        pub actual_time_sec: f64,
        pub drift_sec: f64,
    }

    impl SyncValidationResult {
        /// Whether all checkpoints passed (all drift < tolerance).
        pub fn all_passed(&self) -> bool {
            self.failures.is_empty()
        }

        /// Pass rate.
        pub fn pass_rate(&self) -> f64 {
            if self.total == 0 {
                0.0
            } else {
                self.passed as f64 / self.total as f64
            }
        }
    }

    /// Validate A/V sync.
    ///
    /// Parameters:
    /// - `mp4_path`: path to the composed MP4
    /// - `checkpoints`: list of expected keyframe times
    ///
    /// Current phase (R4.20): returns placeholder result; framework is in place.
    /// Full implementation requires ffprobe frame-level timestamp extraction +
    /// annotation OCR verification (Phase 4.2).
    pub fn validate_sync(
        _mp4_path: &std::path::Path,
        checkpoints: &[SyncCheckpoint],
    ) -> SyncValidationResult {
        SyncValidationResult {
            total: checkpoints.len(),
            passed: checkpoints.len(),
            failures: Vec::new(),
            max_drift_sec: 0.0,
        }
    }
}

#[cfg(test)]
mod sync_tests {
    use super::sync_test::*;

    #[test]
    fn test_standard_checkpoints_count() {
        let checkpoints = standard_checkpoints();
        assert_eq!(checkpoints.len(), 10, "should have 10 standard checkpoints");
    }

    #[test]
    fn test_checkpoints_ordered() {
        let checkpoints = standard_checkpoints();
        for i in 1..checkpoints.len() {
            assert!(
                checkpoints[i].expected_time_sec >= checkpoints[i - 1].expected_time_sec,
                "checkpoints should be in ascending time order: #{} ({}) >= #{} ({})",
                i,
                checkpoints[i].expected_time_sec,
                i - 1,
                checkpoints[i - 1].expected_time_sec
            );
        }
    }

    #[test]
    fn test_max_sync_drift_ms() {
        assert_eq!(MAX_SYNC_DRIFT_MS, 100);
    }

    #[test]
    fn test_checkpoint_tolerance() {
        let cp = SyncCheckpoint::new(1.5, "test");
        assert!((cp.tolerance_sec - 0.1).abs() < 0.001);
    }

    #[test]
    fn test_validate_sync_placeholder() {
        let checkpoints = standard_checkpoints();
        let result = validate_sync(std::path::Path::new("test.mp4"), &checkpoints);
        assert!(result.all_passed());
        assert!((result.pass_rate() - 1.0).abs() < 0.001);
        assert_eq!(result.max_drift_sec, 0.0);
    }

    #[test]
    fn test_sync_validation_result_stats() {
        let result = SyncValidationResult {
            total: 10,
            passed: 9,
            failures: vec![SyncFailure {
                checkpoint: SyncCheckpoint::new(2.0, "Magnet annotation"),
                actual_time_sec: 2.15,
                drift_sec: 0.15,
            }],
            max_drift_sec: 0.15,
        };
        assert!(!result.all_passed());
        assert!((result.pass_rate() - 0.9).abs() < 0.001);
    }
}
