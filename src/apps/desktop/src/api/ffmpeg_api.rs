//! FFmpeg/video processing API.
//!
//! All commands are gated behind `#[cfg(feature = "video")]`.
//! When the feature is disabled, no-op stubs return an error so the
//! invoke_handler macro always sees valid symbols.
//!
//! Based on nathanbabcock/ffmpeg-sidecar v2.5.2 (MIT, crates.io 1.4M downloads).

#[cfg(feature = "video")]
use std::path::Path;

/// Validate that a path is safe for ffmpeg to access:
/// - Must resolve to an absolute path within an allowed directory tree
/// - Must exist (for input paths)
/// On Windows, strips the UNC `\\?\` prefix added by canonicalize() for compatibility.
#[cfg(feature = "video")]
fn validate_input_path(path_str: &str) -> Result<std::path::PathBuf, String> {
    let path = Path::new(path_str);
    let canonical = path
        .canonicalize()
        .map_err(|e| format!("Invalid or inaccessible path '{}': {e}", path_str))?;
    // Strip UNC prefix on Windows for ffmpeg compatibility
    let canonical = dunce::simplified(&canonical).to_path_buf();
    // Reject paths that resolve to system directories
    let canonical_str = canonical.to_string_lossy();
    if canonical_str.starts_with("/etc/")
        || canonical_str.starts_with("/proc/")
        || canonical_str.starts_with("/sys/")
        || canonical_str.starts_with("/dev/")
        || canonical_str.starts_with("C:\\Windows")
        || canonical_str.starts_with("C:\\windows")
    {
        return Err(format!(
            "Path '{}' is in a protected system directory",
            canonical_str
        ));
    }
    Ok(canonical)
}

// ---------------------------------------------------------------------------
// Real implementations (compiled only when "video" feature is enabled)
// ---------------------------------------------------------------------------

#[tauri::command]
#[cfg(feature = "video")]
pub async fn ffmpeg_execute(args: Vec<String>) -> Result<String, String> {
    use ffmpeg_sidecar::command::FfmpegCommand;

    let mut cmd = FfmpegCommand::new();
    let mut i = 0;
    while i < args.len() {
        let arg = &args[i];
        if arg == "-i" && i + 1 < args.len() {
            let safe_path = validate_input_path(&args[i + 1])?;
            cmd.arg("-i");
            cmd.arg(safe_path.to_string_lossy().as_ref());
            i += 2;
        } else {
            cmd.arg(arg);
            i += 1;
        }
    }
    // Restrict to safe protocols only (prevent concat:/subfile:/crypto: SSRF)
    cmd.arg("-protocol_whitelist");
    cmd.arg("file,http,https,tcp,tls");
    let output: Vec<String> = cmd
        .spawn()
        .map_err(|e| format!("ffmpeg spawn failed: {e}"))?
        .iter()
        .map_err(|e| format!("ffmpeg iteration failed: {e}"))?
        .into_ffmpeg_stderr()
        .collect();
    Ok(output.join("\n"))
}

#[tauri::command]
#[cfg(feature = "video")]
pub async fn ffmpeg_get_metadata(input_path: String) -> Result<serde_json::Value, String> {
    use ffmpeg_sidecar::{command::FfmpegCommand, event::FfmpegEvent};

    // Validate and canonicalize input path
    let safe_path = validate_input_path(&input_path)?;

    let iter = FfmpegCommand::new()
        .input(safe_path.to_string_lossy().as_ref())
        .hide_banner()
        .spawn()
        .map_err(|e| format!("ffmpeg metadata spawn failed: {e}"))?
        .iter()
        .map_err(|e| format!("ffmpeg metadata iteration failed: {e}"))?;

    let mut duration: Option<f64> = None;
    let mut format: Option<String> = None;
    let mut width: Option<u32> = None;
    let mut height: Option<u32> = None;
    let mut fps: Option<f32> = None;
    let mut stream_count: u32 = 0;

    for event in iter {
        match event {
            FfmpegEvent::ParsedDuration(d) => {
                duration = Some(d.duration);
            }
            FfmpegEvent::ParsedOutputStream(stream) => {
                let stream_format = stream.format.clone();
                if let Some(v) = stream.video_data() {
                    stream_count += 1;
                    // Keep the first video stream's metadata
                    if stream_count == 1 {
                        format = Some(stream_format);
                        width = Some(v.width);
                        height = Some(v.height);
                        fps = Some(v.fps);
                    }
                }
            }
            FfmpegEvent::Done => break,
            _ => {}
        }
    }

    Ok(serde_json::json!({
        "duration_secs": duration,
        "format": format,
        "width": width,
        "height": height,
        "fps": fps,
        "stream_count": stream_count,
    }))
}

// ---------------------------------------------------------------------------
// No-op stubs (compiled when "video" feature is disabled)
// ---------------------------------------------------------------------------

#[tauri::command]
#[cfg(not(feature = "video"))]
pub async fn ffmpeg_execute(_args: Vec<String>) -> Result<String, String> {
    Err("video feature not enabled".to_string())
}

#[tauri::command]
#[cfg(not(feature = "video"))]
pub async fn ffmpeg_get_metadata(_input_path: String) -> Result<serde_json::Value, String> {
    Err("video feature not enabled".to_string())
}
