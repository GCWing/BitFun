use std::time::Duration;

use tauri::{PhysicalPosition, PhysicalSize, Position, Size};

use crate::executor::BridgeExecutor;
use crate::platform::WindowRect;
use crate::server::response::WebDriverErrorResponse;

impl BridgeExecutor {
    pub(crate) async fn get_window_rect(&self) -> Result<WindowRect, WebDriverErrorResponse> {
        #[cfg(not(target_env = "ohos"))]
        {
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

        #[cfg(target_env = "ohos")]
        {
            Err(WebDriverErrorResponse::unknown_error("Unsupported OS".to_owned()))
        }

    }

    pub(crate) async fn set_window_rect(
        &self,
        rect: WindowRect,
    ) -> Result<WindowRect, WebDriverErrorResponse> {
        #[cfg(not(target_env = "ohos"))]
        {
            let window = self.webview_window()?;

            if window.is_fullscreen().unwrap_or(false) {
                let _ = window.set_fullscreen(false);
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
            if window.is_maximized().unwrap_or(false) {
                let _ = window.unmaximize();
                tokio::time::sleep(Duration::from_millis(50)).await;
            }

            window
                .set_position(Position::Physical(PhysicalPosition::new(rect.x, rect.y)))
                .map_err(|error| {
                    WebDriverErrorResponse::unknown_error(format!(
                        "Failed to set window position: {error}"
                    ))
                })?;

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
            window
                .set_size(Size::Physical(PhysicalSize::new(inner_width, inner_height)))
                .map_err(|error| {
                    WebDriverErrorResponse::unknown_error(format!("Failed to set window size: {error}"))
                })?;

            self.get_window_rect().await
        }

        #[cfg(target_env = "ohos")]
        {
            Err(WebDriverErrorResponse::unknown_error("Unsupported OS".to_owned()))
        }
    }

    pub(crate) async fn maximize_window(&self) -> Result<WindowRect, WebDriverErrorResponse> {
        #[cfg(not(target_env = "ohos"))]
        {
            self.webview_window()?.maximize().map_err(|error| {
                WebDriverErrorResponse::unknown_error(format!("Failed to maximize window: {error}"))
            })?;
            tokio::time::sleep(Duration::from_millis(100)).await;
            self.get_window_rect().await
        }


        #[cfg(target_env = "ohos")]
        {
            Err(WebDriverErrorResponse::unknown_error("Unsupported OS".to_owned()))
        }
    }

    pub(crate) async fn minimize_window(&self) -> Result<(), WebDriverErrorResponse> {
        #[cfg(not(target_env = "ohos"))]
        {
            self.webview_window()?.minimize().map_err(|error| {
                WebDriverErrorResponse::unknown_error(format!("Failed to minimize window: {error}"))
            })?;
            Ok(())
        }

        #[cfg(target_env = "ohos")]
        {
            Err(WebDriverErrorResponse::unknown_error("Unsupported OS".to_owned()))
        }

    }

    pub(crate) async fn fullscreen_window(&self) -> Result<WindowRect, WebDriverErrorResponse> {
        #[cfg(not(target_env = "ohos"))]
        {
            self.webview_window()?
                .set_fullscreen(true)
                .map_err(|error| {
                    WebDriverErrorResponse::unknown_error(format!(
                        "Failed to fullscreen window: {error}"
                    ))
                })?;
            tokio::time::sleep(Duration::from_millis(100)).await;
            self.get_window_rect().await
        }
        #[cfg(target_env = "ohos")]
        {
            Err(WebDriverErrorResponse::unknown_error("Unsupported OS".to_owned()))
        }
    }
}
