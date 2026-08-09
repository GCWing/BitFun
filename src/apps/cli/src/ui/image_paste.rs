use std::io::{Cursor, Read as _};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use image::ImageEncoder as _;

use super::composer::ComposerImage;

const MAX_IMAGE_BYTES: u64 = 20 * 1024 * 1024;
const MAX_IMAGE_DIMENSION: u32 = 16_384;
const MAX_DECODE_ALLOC_BYTES: u64 = 64 * 1024 * 1024;

struct BoundedBytes {
    bytes: Vec<u8>,
    limit: usize,
    exceeded: bool,
}

impl BoundedBytes {
    fn new(limit: usize) -> Self {
        Self {
            bytes: Vec::new(),
            limit,
            exceeded: false,
        }
    }
}

impl std::io::Write for BoundedBytes {
    fn write(&mut self, buffer: &[u8]) -> std::io::Result<usize> {
        if buffer.len() > self.limit.saturating_sub(self.bytes.len()) {
            self.exceeded = true;
            return Err(std::io::Error::other(
                "encoded image exceeds the attachment limit",
            ));
        }
        self.bytes.extend_from_slice(buffer);
        Ok(buffer.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

#[derive(Debug)]
pub(crate) enum ImagePaste {
    Image(ComposerImage),
    Text(String),
}

#[derive(Debug, thiserror::Error)]
pub(crate) enum ImagePasteError {
    #[error("Could not access the clipboard: {0}")]
    Clipboard(String),
    #[error("Could not read image {name}: {message}")]
    ReadImage { name: String, message: String },
    #[error("Image {name} exceeds the 20 MiB attachment limit")]
    TooLarge { name: String },
    #[error("Image {name} is not a valid PNG, JPEG, GIF, or WebP file: {message}")]
    InvalidImage { name: String, message: String },
}

pub(crate) fn read_clipboard(cwd: &Path) -> Result<Option<ImagePaste>, ImagePasteError> {
    let mut clipboard =
        arboard::Clipboard::new().map_err(|error| ImagePasteError::Clipboard(error.to_string()))?;
    match clipboard.get_image() {
        Ok(image) => {
            return image_from_rgba(
                "clipboard.png",
                image.width,
                image.height,
                image.bytes.as_ref(),
            )
            .map(ImagePaste::Image)
            .map(Some);
        }
        Err(arboard::Error::ContentNotAvailable) => {}
        Err(error) => return Err(ImagePasteError::Clipboard(error.to_string())),
    }
    match clipboard.get_text() {
        Ok(text) if !text.is_empty() => classify_pasted_text(&text, cwd).map(Some),
        Ok(_) | Err(arboard::Error::ContentNotAvailable) => Ok(None),
        Err(error) => Err(ImagePasteError::Clipboard(error.to_string())),
    }
}

pub(crate) fn classify_pasted_text(text: &str, cwd: &Path) -> Result<ImagePaste, ImagePasteError> {
    let Some(candidate) = normalize_pasted_path(text, cfg!(windows)) else {
        return Ok(ImagePaste::Text(text.to_string()));
    };
    if !has_supported_image_extension(&candidate) {
        return Ok(ImagePaste::Text(text.to_string()));
    }
    let path = if candidate.is_absolute() {
        candidate
    } else {
        cwd.join(candidate)
    };
    let name = image_name(&path);
    let file = match std::fs::File::open(&path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(ImagePaste::Text(text.to_string()));
        }
        Err(error) => {
            return Err(ImagePasteError::ReadImage {
                name,
                message: error.to_string(),
            });
        }
    };
    let metadata = match file.metadata() {
        Ok(metadata) if metadata.is_file() => metadata,
        Ok(_) => return Ok(ImagePaste::Text(text.to_string())),
        Err(error) => {
            return Err(ImagePasteError::ReadImage {
                name,
                message: error.to_string(),
            });
        }
    };
    if metadata.len() > MAX_IMAGE_BYTES {
        return Err(ImagePasteError::TooLarge { name });
    }
    let mut bytes = Vec::with_capacity(metadata.len() as usize);
    file.take(MAX_IMAGE_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| ImagePasteError::ReadImage {
            name: name.clone(),
            message: error.to_string(),
        })?;
    image_from_bytes(name, bytes).map(ImagePaste::Image)
}

fn normalize_pasted_path(value: &str, windows: bool) -> Option<PathBuf> {
    let value = value.trim();
    if value.is_empty() || value.contains(['\r', '\n']) {
        return None;
    }
    let raw = value.trim_matches(|character| character == '\'' || character == '"');
    if raw.is_empty() {
        return None;
    }
    if raw.starts_with("file://") {
        return url::Url::parse(raw).ok()?.to_file_path().ok();
    }
    if windows {
        return Some(PathBuf::from(raw));
    }
    let mut unescaped = String::with_capacity(raw.len());
    let mut characters = raw.chars();
    while let Some(character) = characters.next() {
        if character == '\\' {
            if let Some(escaped) = characters.next() {
                unescaped.push(escaped);
            } else {
                unescaped.push(character);
            }
        } else {
            unescaped.push(character);
        }
    }
    Some(PathBuf::from(unescaped))
}

fn has_supported_image_extension(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            matches!(
                extension.to_ascii_lowercase().as_str(),
                "png" | "jpg" | "jpeg" | "gif" | "webp"
            )
        })
}

fn image_name(path: &Path) -> String {
    path.file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .unwrap_or("image")
        .to_string()
}

fn image_from_bytes(name: String, bytes: Vec<u8>) -> Result<ComposerImage, ImagePasteError> {
    if bytes.is_empty() {
        return Err(ImagePasteError::InvalidImage {
            name,
            message: "the file is empty".to_string(),
        });
    }
    if bytes.len() as u64 > MAX_IMAGE_BYTES {
        return Err(ImagePasteError::TooLarge { name });
    }
    let format = image::guess_format(&bytes).map_err(|error| ImagePasteError::InvalidImage {
        name: name.clone(),
        message: error.to_string(),
    })?;
    let mime_type = match format {
        image::ImageFormat::Png => "image/png",
        image::ImageFormat::Jpeg => "image/jpeg",
        image::ImageFormat::Gif => "image/gif",
        image::ImageFormat::WebP => "image/webp",
        other => {
            return Err(ImagePasteError::InvalidImage {
                name,
                message: format!("unsupported format {other:?}"),
            });
        }
    };
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(MAX_IMAGE_DIMENSION);
    limits.max_image_height = Some(MAX_IMAGE_DIMENSION);
    limits.max_alloc = Some(MAX_DECODE_ALLOC_BYTES);
    let mut reader = image::ImageReader::with_format(Cursor::new(&bytes), format);
    reader.limits(limits);
    reader
        .decode()
        .map_err(|error| ImagePasteError::InvalidImage {
            name: name.clone(),
            message: error.to_string(),
        })?;
    Ok(ComposerImage::new(
        uuid::Uuid::new_v4().to_string(),
        name,
        mime_type,
        Arc::from(bytes),
    ))
}

fn image_from_rgba(
    name: &str,
    width: usize,
    height: usize,
    rgba: &[u8],
) -> Result<ComposerImage, ImagePasteError> {
    let name = name.to_string();
    let width = u32::try_from(width).map_err(|error| ImagePasteError::InvalidImage {
        name: name.clone(),
        message: error.to_string(),
    })?;
    let height = u32::try_from(height).map_err(|error| ImagePasteError::InvalidImage {
        name: name.clone(),
        message: error.to_string(),
    })?;
    if width == 0
        || height == 0
        || width > MAX_IMAGE_DIMENSION
        || height > MAX_IMAGE_DIMENSION
        || u64::from(width)
            .saturating_mul(u64::from(height))
            .saturating_mul(4)
            > MAX_DECODE_ALLOC_BYTES
    {
        return Err(ImagePasteError::InvalidImage {
            name,
            message: "clipboard image dimensions exceed the attachment limits".to_string(),
        });
    }
    let expected = usize::try_from(width)
        .ok()
        .and_then(|width| {
            usize::try_from(height)
                .ok()
                .and_then(|height| width.checked_mul(height))
        })
        .and_then(|pixels| pixels.checked_mul(4));
    if expected != Some(rgba.len()) {
        return Err(ImagePasteError::InvalidImage {
            name,
            message: "clipboard pixel buffer has invalid dimensions".to_string(),
        });
    }
    let mut png = BoundedBytes::new(MAX_IMAGE_BYTES as usize);
    let encoded = image::codecs::png::PngEncoder::new(&mut png).write_image(
        rgba,
        width,
        height,
        image::ExtendedColorType::Rgba8,
    );
    if png.exceeded {
        return Err(ImagePasteError::TooLarge { name });
    }
    encoded.map_err(|error| ImagePasteError::InvalidImage {
        name: name.clone(),
        message: error.to_string(),
    })?;
    image_from_bytes(name, png.bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{DynamicImage, ImageFormat, RgbaImage};
    use std::io::Cursor;
    use std::io::Write as _;

    fn png_bytes() -> Vec<u8> {
        let mut bytes = Vec::new();
        DynamicImage::ImageRgba8(RgbaImage::from_pixel(2, 1, image::Rgba([1, 2, 3, 255])))
            .write_to(&mut Cursor::new(&mut bytes), ImageFormat::Png)
            .unwrap();
        bytes
    }

    #[test]
    fn pasted_path_normalization_matches_opencode_quotes_urls_and_posix_escapes() {
        assert_eq!(
            normalize_pasted_path("'folder/image.png'", false).unwrap(),
            std::path::PathBuf::from("folder/image.png")
        );
        assert_eq!(
            normalize_pasted_path("folder/my\\ image.png", false).unwrap(),
            std::path::PathBuf::from("folder/my image.png")
        );
        let (file_url, expected) = if cfg!(windows) {
            (
                "file:///C:/tmp/my%20image.png",
                std::path::PathBuf::from(r"C:\tmp\my image.png"),
            )
        } else {
            (
                "file:///tmp/my%20image.png",
                std::path::PathBuf::from("/tmp/my image.png"),
            )
        };
        assert_eq!(
            normalize_pasted_path(file_url, cfg!(windows)).unwrap(),
            expected
        );
    }

    #[test]
    fn valid_file_image_is_snapshotted_with_magic_derived_mime() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("sample.png");
        std::fs::write(&path, png_bytes()).unwrap();

        let paste = classify_pasted_text(path.to_string_lossy().as_ref(), dir.path()).unwrap();
        let ImagePaste::Image(image) = paste else {
            panic!("expected image paste");
        };
        assert_eq!(image.name, "sample.png");
        assert_eq!(image.mime_type, "image/png");
    }

    #[test]
    fn invalid_supported_extension_is_reported_instead_of_attached() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("broken.png");
        std::fs::write(&path, b"not an image").unwrap();

        let error = classify_pasted_text(path.to_string_lossy().as_ref(), dir.path()).unwrap_err();

        assert!(matches!(error, ImagePasteError::InvalidImage { .. }));
    }

    #[test]
    fn oversized_image_is_rejected_before_its_contents_are_read() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("oversized.png");
        let file = std::fs::File::create(&path).unwrap();
        file.set_len(MAX_IMAGE_BYTES + 1).unwrap();

        let error = classify_pasted_text(path.to_string_lossy().as_ref(), dir.path()).unwrap_err();

        assert!(matches!(error, ImagePasteError::TooLarge { .. }));
    }

    #[test]
    fn ordinary_text_and_unsupported_file_paths_remain_text() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("notes.txt");
        std::fs::write(&path, "hello").unwrap();

        assert!(matches!(
            classify_pasted_text("hello world", dir.path()).unwrap(),
            ImagePaste::Text(text) if text == "hello world"
        ));
        assert!(matches!(
            classify_pasted_text(path.to_string_lossy().as_ref(), dir.path()).unwrap(),
            ImagePaste::Text(_)
        ));
    }

    #[test]
    fn rgba_clipboard_pixels_are_encoded_as_a_valid_png_snapshot() {
        let image = image_from_rgba("clipboard.png", 1, 1, &[10, 20, 30, 255]).unwrap();

        assert_eq!(image.mime_type, "image/png");
        assert_eq!(
            image::guess_format(image.bytes()).unwrap(),
            ImageFormat::Png
        );
    }

    #[test]
    fn clipboard_dimensions_are_rejected_before_png_encoding() {
        let width = MAX_IMAGE_DIMENSION as usize + 1;
        let pixels = vec![0; width * 4];

        let error = image_from_rgba("clipboard.png", width, 1, &pixels).unwrap_err();

        assert!(matches!(
            error,
            ImagePasteError::InvalidImage { message, .. }
                if message == "clipboard image dimensions exceed the attachment limits"
        ));
    }

    #[test]
    fn encoded_clipboard_output_is_bounded_while_the_encoder_is_writing() {
        let mut output = BoundedBytes::new(3);

        let error = output.write_all(&[1, 2, 3, 4]).unwrap_err();

        assert_eq!(error.kind(), std::io::ErrorKind::Other);
        assert!(output.exceeded);
        assert!(output.bytes.is_empty());
    }
}
