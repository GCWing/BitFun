//! Local, provider-neutral document-to-Markdown conversion for the Read tool.

use std::collections::VecDeque;
use std::fmt;
use std::path::Path;
use std::sync::{Arc, Mutex, OnceLock};

use anydoc::Format;
use sha2::{Digest, Sha256};
use tokio::sync::Semaphore;

/// Maximum source-document size accepted by the Read tool conversion path.
pub const MAX_DOCUMENT_INPUT_BYTES: usize = 64 * 1024 * 1024;

/// Maximum retained Markdown for one conversion and across the in-memory conversion cache.
pub const MAX_DOCUMENT_MARKDOWN_BYTES: usize = 16 * 1024 * 1024;

const MAX_DOCUMENT_CACHE_ENTRIES: usize = 4;

/// A document representation that can be paged by the normal Read primitives.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConvertedDocument {
    pub markdown: Arc<str>,
    pub source_format: &'static str,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct DocumentCacheKey {
    source_sha256: [u8; 32],
    format: Format,
}

struct DocumentCacheEntry {
    key: DocumentCacheKey,
    document: ConvertedDocument,
}

#[derive(Default)]
struct DocumentCache {
    entries: VecDeque<DocumentCacheEntry>,
    retained_markdown_bytes: usize,
}

impl DocumentCache {
    fn get(&mut self, key: DocumentCacheKey) -> Option<ConvertedDocument> {
        let index = self.entries.iter().position(|entry| entry.key == key)?;
        let entry = self.entries.remove(index)?;
        let document = entry.document.clone();
        self.entries.push_back(entry);
        Some(document)
    }

    fn insert(&mut self, key: DocumentCacheKey, document: ConvertedDocument) {
        let markdown_bytes = document.markdown.len();
        if markdown_bytes > MAX_DOCUMENT_MARKDOWN_BYTES {
            return;
        }

        while self.entries.len() >= MAX_DOCUMENT_CACHE_ENTRIES
            || self.retained_markdown_bytes.saturating_add(markdown_bytes)
                > MAX_DOCUMENT_MARKDOWN_BYTES
        {
            let Some(evicted) = self.entries.pop_front() else {
                break;
            };
            self.retained_markdown_bytes = self
                .retained_markdown_bytes
                .saturating_sub(evicted.document.markdown.len());
        }

        self.retained_markdown_bytes = self.retained_markdown_bytes.saturating_add(markdown_bytes);
        self.entries.push_back(DocumentCacheEntry { key, document });
    }
}

/// Provider-neutral document conversion failure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentConversionError {
    code: &'static str,
    message: String,
}

impl DocumentConversionError {
    fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }

    pub fn code(&self) -> &'static str {
        self.code
    }
}

impl fmt::Display for DocumentConversionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for DocumentConversionError {}

/// Whether the path extension names a format handled by anydoc.
pub fn is_supported_document_path(path: &str) -> bool {
    Format::from_path(Path::new(path)).is_some()
}

/// Convert document bytes on the blocking pool. Conversion is serialized process-wide because
/// parsers can temporarily retain substantially more decompressed data than the source file.
pub async fn convert_document_to_markdown(
    bytes: Vec<u8>,
    path_hint: String,
) -> Result<ConvertedDocument, DocumentConversionError> {
    if bytes.len() > MAX_DOCUMENT_INPUT_BYTES {
        return Err(DocumentConversionError::new(
            "resourceLimit",
            format!(
                "document is larger than the {} MiB Read limit",
                MAX_DOCUMENT_INPUT_BYTES / (1024 * 1024)
            ),
        ));
    }

    let permit = document_conversion_semaphore()
        .clone()
        .acquire_owned()
        .await
        .map_err(|_| {
            DocumentConversionError::new(
                "runtime",
                "document conversion is unavailable because its worker was closed",
            )
        })?;

    tokio::task::spawn_blocking(move || {
        // Keep the permit inside the blocking task. If the async caller is cancelled, the parser
        // still occupies its bounded slot until the synchronous conversion actually exits.
        let _permit = permit;
        convert_document_to_markdown_sync(&bytes, &path_hint)
    })
    .await
    .map_err(|error| {
        DocumentConversionError::new(
            "runtime",
            format!("document conversion worker failed: {error}"),
        )
    })?
}

fn document_conversion_semaphore() -> &'static Arc<Semaphore> {
    static SEMAPHORE: OnceLock<Arc<Semaphore>> = OnceLock::new();
    SEMAPHORE.get_or_init(|| Arc::new(Semaphore::new(1)))
}

fn convert_document_to_markdown_sync(
    bytes: &[u8],
    path_hint: &str,
) -> Result<ConvertedDocument, DocumentConversionError> {
    let format = Format::from_bytes(bytes)
        .or_else(|| Format::from_path(Path::new(path_hint)))
        .ok_or_else(|| {
            DocumentConversionError::new(
                "unsupported",
                "file content and extension do not identify a supported document format",
            )
        })?;
    let cache_key = DocumentCacheKey {
        source_sha256: Sha256::digest(bytes).into(),
        format,
    };
    if let Some(document) = document_cache()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .get(cache_key)
    {
        return Ok(document);
    }

    let source_format = format_name(format);
    let markdown = anydoc::to_markdown_bytes(bytes, format)
        .map_err(|error| DocumentConversionError::new(error.code(), error.to_string()))?;
    if markdown.len() > MAX_DOCUMENT_MARKDOWN_BYTES {
        return Err(DocumentConversionError::new(
            "resourceLimit",
            format!(
                "converted Markdown is larger than the {} MiB Read limit",
                MAX_DOCUMENT_MARKDOWN_BYTES / (1024 * 1024)
            ),
        ));
    }

    let document = ConvertedDocument {
        markdown: Arc::from(markdown),
        source_format,
    };
    document_cache()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .insert(cache_key, document.clone());
    Ok(document)
}

fn document_cache() -> &'static Mutex<DocumentCache> {
    static CACHE: OnceLock<Mutex<DocumentCache>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(DocumentCache::default()))
}

fn format_name(format: Format) -> &'static str {
    match format {
        Format::Doc => "doc",
        Format::Docx => "docx",
        Format::Odt => "odt",
        Format::Pdf => "pdf",
        Format::Ppt => "ppt",
        Format::Pptx => "pptx",
        Format::Rtf => "rtf",
        Format::Epub => "epub",
        Format::Excel => "excel",
        Format::Ods => "ods",
        Format::Odp => "odp",
        Format::Csv => "csv",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognizes_all_supported_extension_families() {
        for path in [
            "report.doc",
            "report.DOCX",
            "report.docm",
            "slides.ppt",
            "slides.ppsx",
            "sheet.xlsb",
            "sheet.xlsx",
            "document.odt",
            "sheet.ods",
            "slides.odp",
            "notes.rtf",
            "book.epub",
            "table.csv",
            "paper.pdf",
        ] {
            assert!(is_supported_document_path(path), "{path}");
        }
        assert!(!is_supported_document_path("src/lib.rs"));
        assert!(!is_supported_document_path("README.md"));
    }

    #[test]
    fn content_detection_takes_precedence_over_a_wrong_extension_hint() {
        let converted =
            convert_document_to_markdown_sync(br"{\rtf1\ansi Hello from RTF}", "mislabelled.pdf")
                .expect("RTF should convert");

        assert_eq!(converted.source_format, "rtf");
        assert!(converted.markdown.contains("Hello from RTF"));
    }

    #[test]
    fn csv_uses_the_path_hint_because_it_has_no_content_signature() {
        let converted =
            convert_document_to_markdown_sync(b"name,value\nalpha,1\nbeta,2\n", "table.csv")
                .expect("CSV should convert");

        assert_eq!(converted.source_format, "csv");
        assert!(converted.markdown.contains("| name | value |"));
        assert!(converted.markdown.contains("| alpha | 1 |"));
    }

    #[test]
    fn repeated_conversion_reuses_cached_markdown_for_offset_reads() {
        let first =
            convert_document_to_markdown_sync(br"{\rtf1\ansi Cached document}", "cached.rtf")
                .expect("first conversion");
        let second =
            convert_document_to_markdown_sync(br"{\rtf1\ansi Cached document}", "cached.rtf")
                .expect("second conversion");

        assert!(Arc::ptr_eq(&first.markdown, &second.markdown));
    }
}
