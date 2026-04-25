use std::time::Duration;

use tauri::{PhysicalPosition, PhysicalSize, Position, Size};

use crate::executor::BridgeExecutor;
use crate::platform::WindowRect;
use crate::server::response::WebDriverErrorResponse;

impl BridgeExecutor {
    pub async fn get_window_rect(&self) -> Result<WindowRect, WebDriverErrorResponse> {
        let window = self.webview_window()?;
        let position = window.outer_position().map_err(|error| {
            WebDriverErrorResponse::unknown_error(format!(
                "Failed to read window position: {error}"
            ))
        })?;
        let size = window.outer_size().map_err(|error| {
            WebDriverErrorResponse::unknown_error(format!("Failed to read window size: {error}"))
        })?;

        Ok(WindowRect {
            x: position.x,
            y: position.y,
            width: size.width,
            height: size.height,
        })
    }

    pub async fn set_window_rect(
        &self,
        rect: WindowRect,
    ) -> Result<WindowRect, WebDriverErrorResponse> {
        let window = self.webview_window()?;

        if window.is_fullscreen().unwrap_or(false) {
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        if window.is_maximized().unwrap_or(false) {
            tokio::time::sleep(Duration::from_millis(50)).await;
        }

        let (chrome_width, chrome_height) =
            if let (Ok(outer), Ok(inner)) = (window.outer_size(), window.inner_size()) {
                (
                    outer.width.saturating_sub(inner.width),
                    outer.height.saturating_sub(inner.height),
                )
            } else {
                (0, 0)
            };

        let inner_width = rect.width.saturating_sub(chrome_width);
        let inner_height = rect.height.saturating_sub(chrome_height);

        self.get_window_rect().await
    }

    pub async fn maximize_window(&self) -> Result<WindowRect, WebDriverErrorResponse> {
        tokio::time::sleep(Duration::from_millis(100)).await;
        self.get_window_rect().await
    }

    pub async fn minimize_window(&self) -> Result<(), WebDriverErrorResponse> {
        Ok(())
    }

    pub async fn fullscreen_window(&self) -> Result<WindowRect, WebDriverErrorResponse> {
        tokio::time::sleep(Duration::from_millis(100)).await;
        self.get_window_rect().await
    }
}
