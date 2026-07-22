use std::collections::HashMap;
use std::io::Write;
use std::process::{Child, ChildStdin, Command, Stdio};

use crate::kline_renderer::KLineRenderer;
use crate::types::bar_types::RawBar;

/// Per-bar live-streaming engine: KLineRenderer → FFmpeg image2pipe → RTMP.
pub struct LiveStreamEngine {
    rtmp_url: String,
    width: u32,
    height: u32,
    fps: u32,
    renderer: KLineRenderer,
    child: Option<Child>,
    stdin: Option<ChildStdin>,
}

impl LiveStreamEngine {
    pub fn new(rtmp_url: &str, width: u32, height: u32, fps: u32) -> Self {
        Self {
            rtmp_url: rtmp_url.to_string(),
            width,
            height,
            fps,
            renderer: KLineRenderer::new(width, height),
            child: None,
            stdin: None,
        }
    }

    /// Spawn the FFmpeg subprocess and connect its stdin via pipe.
    ///
    /// FFmpeg command:
    /// ```text
    /// ffmpeg -y -f image2pipe -framerate {fps} -i - \
    ///        -c:v libx264 -preset veryfast -tune zerolatency \
    ///        -pix_fmt yuv420p -f flv {rtmp_url}
    /// ```
    pub fn start(&mut self) -> Result<(), String> {
        if self.child.is_some() {
            return Err("LiveStreamEngine already started".into());
        }

        let mut cmd = Command::new("ffmpeg");

        cmd.arg("-y")
            .arg("-f")
            .arg("image2pipe")
            .arg("-framerate")
            .arg(self.fps.to_string())
            .arg("-i")
            .arg("-") // stdin
            .arg("-c:v")
            .arg("libx264")
            .arg("-preset")
            .arg("veryfast")
            .arg("-tune")
            .arg("zerolatency")
            .arg("-pix_fmt")
            .arg("yuv420p")
            .arg("-f")
            .arg("flv")
            .arg(&self.rtmp_url);

        cmd.stdin(Stdio::piped());
        cmd.stderr(Stdio::piped());
        cmd.stdout(Stdio::null());

        let mut child = cmd
            .spawn()
            .map_err(|e| format!("Failed to spawn ffmpeg: {}", e))?;

        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| "Failed to capture ffmpeg stdin".to_string())?;

        self.child = Some(child);
        self.stdin = Some(stdin);

        log::info!(
            "LiveStreamEngine started: {}x{} @ {}fps → {}",
            self.width,
            self.height,
            self.fps,
            self.rtmp_url
        );

        Ok(())
    }

    /// Push one bar frame into the stream.
    ///
    /// Renders the bar + indicators via `KLineRenderer`, then writes the PNG
    /// bytes to FFmpeg's stdin pipe.
    pub fn push_bar(
        &mut self,
        bar: &RawBar,
        indicators: &HashMap<String, f64>,
    ) -> Result<(), String> {
        let stdin = self
            .stdin
            .as_mut()
            .ok_or_else(|| "LiveStreamEngine not started".to_string())?;

        // For streaming we don't keep a history; pass empty prev_bars.
        let png_bytes = self.renderer.render_bar(bar, &[], indicators)?;

        stdin
            .write_all(&png_bytes)
            .map_err(|e| format!("Failed to write frame to ffmpeg pipe: {}", e))?;
        stdin
            .flush()
            .map_err(|e| format!("Failed to flush ffmpeg pipe: {}", e))?;

        Ok(())
    }

    /// Stop the stream: close stdin and wait for FFmpeg to exit.
    pub fn stop(&mut self) -> Result<(), String> {
        // Drop stdin first to signal EOF to ffmpeg.
        self.stdin = None;

        if let Some(mut child) = self.child.take() {
            let status = child
                .wait()
                .map_err(|e| format!("Failed to wait on ffmpeg: {}", e))?;

            if !status.success() {
                // Read stderr for diagnostics.
                let stderr_output = child
                    .stderr
                    .take()
                    .map(|mut r| {
                        use std::io::Read;
                        let mut s = String::new();
                        let _ = r.read_to_string(&mut s);
                        s
                    })
                    .unwrap_or_default();

                log::error!(
                    "FFmpeg exited with status {:?}: {}",
                    status.code(),
                    stderr_output
                );
                return Err(format!(
                    "FFmpeg exited with status {:?}: {}",
                    status.code(),
                    stderr_output
                ));
            }

            log::info!("LiveStreamEngine stopped cleanly");
        }

        Ok(())
    }

    /// Whether the engine has been started and is running.
    pub fn is_running(&self) -> bool {
        self.stdin.is_some()
    }
}

impl Drop for LiveStreamEngine {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn make_bar(open: f64, high: f64, low: f64, close: f64, vol: f64) -> RawBar {
        RawBar {
            symbol: "rb9999".into(),
            dt: chrono::Utc.with_ymd_and_hms(2026, 7, 22, 9, 30, 0).unwrap(),
            open,
            high,
            low,
            close,
            vol,
        }
    }

    #[test]
    fn test_constructor() {
        let engine = LiveStreamEngine::new("rtmp://localhost/live/test", 640, 480, 25);
        assert_eq!(engine.rtmp_url, "rtmp://localhost/live/test");
        assert_eq!(engine.width, 640);
        assert_eq!(engine.height, 480);
        assert_eq!(engine.fps, 25);
        assert!(!engine.is_running());
    }

    #[test]
    fn test_start_twice_errors() {
        // Use a non-existent ffmpeg so spawn fails (we only test the state guard).
        let mut engine = LiveStreamEngine::new("rtmp://localhost/live/test", 640, 480, 25);
        // start() may fail because ffmpeg isn't on PATH — that's fine.
        // What we test: if start succeeds, calling it again should error.
        let first = engine.start();
        if first.is_ok() {
            let second = engine.start();
            assert!(second.is_err());
            assert!(second.unwrap_err().contains("already started"));
            let _ = engine.stop();
        }
        // If ffmpeg isn't installed, start() fails — still acceptable.
    }

    #[test]
    fn test_push_bar_before_start_errors() {
        let mut engine = LiveStreamEngine::new("rtmp://localhost/live/test", 640, 480, 25);
        let bar = make_bar(4000.0, 4020.0, 3980.0, 4010.0, 5000.0);
        let result = engine.push_bar(&bar, &HashMap::new());
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not started"));
    }

    #[test]
    fn test_ffmpeg_not_found() {
        let result = std::process::Command::new("nonexistent_ffmpeg_binary_xyz")
            .arg("--version")
            .output();
        assert!(result.is_err());
    }

    #[test]
    fn test_drop_stops_engine() {
        let mut engine = LiveStreamEngine::new("rtmp://localhost/live/test", 640, 480, 25);
        // If ffmpeg is available and start succeeds, drop should clean up.
        // If ffmpeg isn't available, this test is still valid — it verifies
        // that the engine doesn't panic on drop.
        let _ = engine.start();
        // engine goes out of scope → Drop::drop calls stop()
    }
}
