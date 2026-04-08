mod capture;
pub(crate) mod evaluator;
mod image;
mod types;

pub use capture::{print_page, take_screenshot};
pub use image::crop_screenshot;
pub use types::{Cookie, ElementScreenshotMetadata, PrintOptions, WindowRect};
