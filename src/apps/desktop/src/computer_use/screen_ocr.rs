use log::{info, warn};
use openbitfun_core::agentic::tools::computer_use_host::ComputerScreenshot;
use openbitfun_core::infrastructure::try_get_path_manager_arc;
use openbitfun_core::util::errors::{OpenBitFunError, OpenBitFunResult};
use std::fs;
use std::io::Write;
use std::path::PathBuf;

use chrono::Utc;

#[cfg(all(test, target_os = "macos"))]
mod native_fixture_tests {
    use super::*;

    /// Exercises the actual Vision backend and global-coordinate projection on
    /// a browser-rendered image, without capturing or operating the user's UI.
    #[test]
    #[ignore = "requires OPENBITFUN_OCR_FIXTURE containing a rendered screenshot DTO"]
    fn native_vision_reads_rendered_fixture() {
        let path = std::env::var("OPENBITFUN_OCR_FIXTURE").expect("rendered OCR fixture path");
        let shot: ComputerScreenshot =
            serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        let matches = macos::find_text_matches(&shot, "Save report").expect("real Vision OCR");
        let hit = &matches[0];
        assert!(hit.text.to_lowercase().contains("save report"), "{hit:?}");
        assert!(hit.confidence > 0.5, "{hit:?}");
        // Text is rendered at x=100..400, y=100..180 in an 800x600 image;
        // the DTO maps the image to (-500,100,400,300) global coordinates.
        assert!((-450.0..-300.0).contains(&hit.center_x), "{hit:?}");
        assert!((150.0..190.0).contains(&hit.center_y), "{hit:?}");
        assert!(hit.bounds_width > 0.0 && hit.bounds_height > 0.0);
    }
}

pub(super) use openbitfun_core::agentic::tools::computer_use_host::OcrTextMatch;

pub(super) fn find_text_matches(
    shot: &ComputerScreenshot,
    text_query: &str,
) -> OpenBitFunResult<Vec<OcrTextMatch>> {
    let query = normalize_query(text_query)?;
    save_ocr_debug_jpeg(shot, &query);

    #[cfg(target_os = "macos")]
    {
        return macos::find_text_matches(shot, &query);
    }

    #[cfg(target_os = "windows")]
    {
        return windows_backend::find_text_matches(shot, &query);
    }

    #[cfg(target_os = "linux")]
    {
        return linux_backend::find_text_matches(shot, &query);
    }

    #[allow(unreachable_code)]
    Err(OpenBitFunError::tool(
        "move_to_text OCR is not supported on this platform.".to_string(),
    ))
}

/// If unset or non-zero: write the exact JPEG passed to OCR into `computer_use_debug` under the app data dir (see implementation). Set `OPENBITFUN_COMPUTER_USE_OCR_DEBUG=0` to disable.
fn ocr_debug_save_enabled() -> bool {
    !matches!(
        std::env::var("OPENBITFUN_COMPUTER_USE_OCR_DEBUG"),
        Ok(v) if v == "0" || v.eq_ignore_ascii_case("false")
    )
}

/// Same directory as agent `screenshot` debug (`workspace/.openbitfun/computer_use_debug`), when PathManager is available.
fn computer_use_ocr_debug_dir() -> PathBuf {
    if let Ok(pm) = try_get_path_manager_arc() {
        return pm
            .default_assistant_workspace_dir(None)
            .join(openbitfun_core_types::product_identity::hidden_data_directory())
            .join("computer_use_debug");
    }
    dirs::home_dir()
        .map(|h| {
            h.join(openbitfun_core_types::product_identity::hidden_data_directory())
                .join("personal_assistant")
                .join("workspace")
                .join(openbitfun_core_types::product_identity::hidden_data_directory())
                .join("computer_use_debug")
        })
        .unwrap_or_else(|| std::env::temp_dir().join("computer_use_debug"))
}

/// Persists `shot.bytes` (same buffer as Vision / WinRT / Tesseract) before OCR runs.
fn save_ocr_debug_jpeg(shot: &ComputerScreenshot, text_query: &str) {
    if !ocr_debug_save_enabled() {
        return;
    }
    let dir = computer_use_ocr_debug_dir();
    if let Err(e) = fs::create_dir_all(&dir) {
        warn!("computer_use ocr_debug: create_dir_all {:?}: {}", dir, e);
        return;
    }
    let safe: String = text_query
        .chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            c if c.is_control() => '_',
            c => c,
        })
        .take(96)
        .collect();
    let safe = if safe.trim().is_empty() {
        "query".to_string()
    } else {
        safe
    };
    let ts = Utc::now().format("%Y%m%d_%H%M%S");
    let ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    let name = format!(
        "ocr_{}_{}_{}x{}_{}ms_{}.jpg",
        ts,
        ms,
        shot.image_width,
        shot.image_height,
        shot.bytes.len(),
        safe
    );
    let path = dir.join(name);
    match fs::File::create(&path).and_then(|mut f| f.write_all(&shot.bytes)) {
        Ok(()) => {
            info!(
                "computer_use ocr_debug: wrote {} bytes to {}",
                shot.bytes.len(),
                path.display()
            );
        }
        Err(e) => warn!("computer_use ocr_debug: write {:?}: {}", path, e),
    }
}

fn normalize_query(text_query: &str) -> OpenBitFunResult<String> {
    let q = text_query.trim();
    if q.is_empty() {
        return Err(OpenBitFunError::tool(
            "move_to_text requires a non-empty text_query.".to_string(),
        ));
    }
    Ok(q.to_string())
}

use super::ocr_context::*;

// ---------------------------------------------------------------------------
// macOS: Vision framework OCR via objc2-vision
// ---------------------------------------------------------------------------
#[cfg(target_os = "macos")]
mod macos {
    use super::{
        filter_and_rank, fuzzy_text_matches_query, image_box_to_global_match,
        image_content_rect_or_full, levenshtein_chars, normalize_for_match, OcrTextMatch,
    };
    use objc2::msg_send;
    use objc2::rc::Retained;
    use objc2::AnyThread;
    use objc2_foundation::{NSArray, NSData, NSDictionary, NSError, NSString};
    use objc2_vision::{
        VNImageOption, VNImageRectForNormalizedRect, VNImageRequestHandler, VNRecognizeTextRequest,
        VNRecognizeTextRequestRevision3, VNRecognizedTextObservation, VNRequest,
        VNRequestTextRecognitionLevel,
    };
    use openbitfun_core::agentic::tools::computer_use_host::ComputerScreenshot;
    use openbitfun_core::util::errors::{OpenBitFunError, OpenBitFunResult};

    /// Top-N candidates per observation; Chinese matches often appear below rank 1.
    const TOP_CANDIDATES_MAX: usize = 10;

    pub(super) fn find_text_matches(
        shot: &ComputerScreenshot,
        text_query: &str,
    ) -> OpenBitFunResult<Vec<OcrTextMatch>> {
        let (_content_left, _content_top, content_width, content_height) =
            image_content_rect_or_full(shot);
        if content_width == 0 || content_height == 0 {
            return Err(OpenBitFunError::tool(
                "Screenshot content rect is empty; cannot run macOS Vision OCR.".to_string(),
            ));
        }

        let observations = recognize_text_observations(&shot.bytes)?;
        let mut raw_matches = Vec::new();
        for obs in &observations {
            if let Some(m) = observation_to_match(shot, text_query, obs) {
                raw_matches.push(m);
            }
        }

        let ranked = filter_and_rank(text_query, raw_matches);
        if ranked.is_empty() {
            return Err(OpenBitFunError::tool(format!(
                "No OCR text matched {:?} on screen (macOS Vision found {} text regions total). \
                 Matching strips whitespace between glyphs and allows small edit distance for OCR errors. \
                 If the UI is Chinese, try a shorter substring or ensure the text is visible in the capture.",
                text_query,
                observations.len()
            )));
        }
        Ok(ranked)
    }

    fn recognize_text_observations(
        jpeg_bytes: &[u8],
    ) -> OpenBitFunResult<Vec<Retained<VNRecognizedTextObservation>>> {
        // Create NSData from the raw JPEG bytes.
        let ns_data = NSData::with_bytes(jpeg_bytes);

        // Create the text recognition request.
        let request = VNRecognizeTextRequest::new();
        // Revision 3: language auto-detection + improved scripts (CJK).
        unsafe {
            let _: () = msg_send![&*request, setRevision: VNRecognizeTextRequestRevision3];
        }
        request.setRecognitionLevel(VNRequestTextRecognitionLevel::Accurate);
        request.setUsesLanguageCorrection(true);
        request.setAutomaticallyDetectsLanguage(true);

        // Prefer Simplified Chinese, Traditional Chinese, then English (WeChat / mixed UIs).
        let zh_hans = NSString::from_str("zh-Hans");
        let zh_hant = NSString::from_str("zh-Hant");
        let en_us = NSString::from_str("en-US");
        let langs = NSArray::from_retained_slice(&[zh_hans, zh_hant, en_us]);
        request.setRecognitionLanguages(&langs);

        request.setMinimumTextHeight(0.005);

        // Upcast VNRecognizeTextRequest -> VNImageBasedRequest -> VNRequest
        // via Retained::into_super() twice.
        let request_as_vn: Retained<VNRequest> =
            Retained::into_super(Retained::into_super(request.clone()));

        let requests = NSArray::from_retained_slice(&[request_as_vn]);

        // Build VNImageRequestHandler from NSData (JPEG).
        let options: Retained<NSDictionary<VNImageOption, objc2::runtime::AnyObject>> =
            NSDictionary::new();
        let handler = VNImageRequestHandler::initWithData_options(
            VNImageRequestHandler::alloc(),
            &ns_data,
            &options,
        );

        // Perform the request synchronously.
        handler
            .performRequests_error(&requests)
            .map_err(ns_error_to_openbitfun)?;

        // Collect results.
        let results = match request.results() {
            Some(arr) => arr,
            None => return Ok(Vec::new()),
        };
        Ok(results.to_vec())
    }

    fn ns_error_to_openbitfun(err: Retained<NSError>) -> OpenBitFunError {
        let desc = err.localizedDescription().to_string();
        OpenBitFunError::tool(format!("macOS Vision OCR failed: {}", desc))
    }

    fn observation_to_match(
        shot: &ComputerScreenshot,
        text_query: &str,
        obs: &VNRecognizedTextObservation,
    ) -> Option<OcrTextMatch> {
        let candidates = obs.topCandidates(TOP_CANDIDATES_MAX);
        let n = candidates.len();
        let q_norm = normalize_for_match(text_query);

        let mut chosen_text: Option<String> = None;
        let mut chosen_confidence: f32 = 0.0;

        for i in 0..n {
            let candidate = unsafe { candidates.objectAtIndex_unchecked(i) };
            let text = candidate.string().to_string();
            if !normalize_for_match(&text).contains(&q_norm) {
                continue;
            }
            let conf = candidate.confidence();
            if chosen_text.is_none() || conf > chosen_confidence {
                chosen_text = Some(text);
                chosen_confidence = conf;
            }
        }

        // Fuzzy fallback: Vision may insert spaces in CJK, mis-read one character, or split labels.
        if chosen_text.is_none() {
            let mut best: Option<(String, f32, usize)> = None;
            for i in 0..n {
                let candidate = unsafe { candidates.objectAtIndex_unchecked(i) };
                let text = candidate.string().to_string();
                if !fuzzy_text_matches_query(&text, text_query) {
                    continue;
                }
                let nt = normalize_for_match(&text);
                let dist = levenshtein_chars(&nt, &q_norm);
                let conf = candidate.confidence();
                let take = match &best {
                    None => true,
                    Some((_, bf, bd)) => dist < *bd || (dist == *bd && conf > *bf),
                };
                if take {
                    best = Some((text, conf, dist));
                }
            }
            if let Some((t, c, _)) = best {
                chosen_text = Some(t);
                chosen_confidence = c;
            }
        }

        let text = chosen_text?;

        // Vision bounding box is normalized to the **full** image (JPEG), not the content rect.
        let bounding = unsafe { obs.boundingBox() };
        let image_rect = unsafe {
            VNImageRectForNormalizedRect(
                bounding,
                shot.image_width as usize,
                shot.image_height as usize,
            )
        };

        // image_rect origin is bottom-left in image pixel space; convert to top-left.
        let local_left = image_rect.origin.x;
        let local_top = shot.image_height as f64 - image_rect.origin.y - image_rect.size.height;
        let width = image_rect.size.width;
        let height = image_rect.size.height;

        image_box_to_global_match(
            shot,
            text,
            chosen_confidence,
            local_left,
            local_top,
            width,
            height,
        )
    }
}

// ---------------------------------------------------------------------------
// Windows: Windows.Media.Ocr UWP API
// ---------------------------------------------------------------------------
#[cfg(target_os = "windows")]
mod windows_backend {
    use super::{
        filter_and_rank, fuzzy_text_matches_query, image_box_to_global_match,
        image_content_rect_or_full, normalize_for_match, OcrTextMatch,
    };
    use openbitfun_core::agentic::tools::computer_use_host::ComputerScreenshot;
    use openbitfun_core::util::errors::{OpenBitFunError, OpenBitFunResult};
    use windows::core::HSTRING;
    use windows::Graphics::Imaging::BitmapDecoder;
    use windows::Media::Ocr::{OcrEngine, OcrWord};
    use windows::Storage::Streams::{DataWriter, InMemoryRandomAccessStream};
    use windows::Win32::System::Com::{
        CoIncrementMTAUsage, CoInitializeEx, CoUninitialize, COINIT_APARTMENTTHREADED,
        COINIT_DISABLE_OLE1DDE,
    };

    fn w<T>(r: windows::core::Result<T>) -> OpenBitFunResult<T> {
        r.map_err(|e| OpenBitFunError::tool(format!("Windows OCR: {}", e)))
    }

    pub(super) fn find_text_matches(
        shot: &ComputerScreenshot,
        text_query: &str,
    ) -> OpenBitFunResult<Vec<OcrTextMatch>> {
        let (content_left, content_top, content_width, content_height) =
            image_content_rect_or_full(shot);
        if content_width == 0 || content_height == 0 {
            return Err(OpenBitFunError::tool(
                "Screenshot content rect is empty; cannot run Windows OCR.".to_string(),
            ));
        }

        // Initialize COM apartment for WinRT APIs
        // This must run on a thread initialized with COINIT_APARTMENTTHREADED
        // Windows.Media.Ocr requires STA thread
        let mut co_init = None;
        if unsafe { CoIncrementMTAUsage() }.is_err() {
            let hr =
                unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED | COINIT_DISABLE_OLE1DDE) };
            if hr.is_err() {
                return Err(OpenBitFunError::tool(format!(
                    "Windows OCR COM initialization failed: {:?}",
                    hr
                )));
            }
            co_init = Some(());
        }

        let result = (|| -> OpenBitFunResult<Vec<OcrTextMatch>> {
            // 1. Write JPEG bytes to in-memory stream
            let stream = w(InMemoryRandomAccessStream::new())?;
            let writer = w(DataWriter::CreateDataWriter(&stream))?;
            w(writer.WriteBytes(&shot.bytes))?;
            w(w(writer.StoreAsync())?.get())?;
            w(w(writer.FlushAsync())?.get())?;
            w(writer.DetachStream())?;

            // 2. Decode JPEG to SoftwareBitmap
            let decoder = w(w(BitmapDecoder::CreateAsync(&stream))?.get())?;
            let software_bitmap = w(w(decoder.GetSoftwareBitmapAsync())?.get())?;

            // 3. Create OCR engine (use user profile languages)
            let engine = match OcrEngine::TryCreateFromUserProfileLanguages() {
                Ok(e) => e,
                Err(_) => {
                    // Fallback to English if user profile languages fail
                    let lang = w(windows::Globalization::Language::CreateLanguage(
                        &HSTRING::from("en-US"),
                    ))?;
                    if !w(OcrEngine::IsLanguageSupported(&lang))? {
                        return Err(OpenBitFunError::tool(
                            "Windows OCR: No supported language packs installed.".to_string(),
                        ));
                    }
                    w(OcrEngine::TryCreateFromLanguage(&lang))?
                }
            };

            // 4. Run OCR recognition
            let ocr_result = w(w(engine.RecognizeAsync(&software_bitmap))?.get())?;
            let lines = w(ocr_result.Lines())?;
            let line_count = w(lines.Size())?;

            let mut raw_matches = Vec::new();
            for line in &lines {
                let words = w(line.Words())?;
                for word in &words {
                    if let Some(m) = ocr_word_to_match(
                        shot,
                        text_query,
                        &word,
                        content_left,
                        content_top,
                        content_width,
                        content_height,
                    ) {
                        raw_matches.push(m);
                    }
                }
            }

            let ranked = filter_and_rank(text_query, raw_matches);
            if ranked.is_empty() {
                return Err(OpenBitFunError::tool(format!(
                    "No OCR text matched {:?} on screen (Windows OCR found {} text regions total).",
                    text_query, line_count
                )));
            }
            Ok(ranked)
        })();

        // Uninitialize COM if we initialized it
        if co_init.is_some() {
            unsafe { CoUninitialize() };
        }

        result
    }

    fn ocr_word_to_match(
        shot: &ComputerScreenshot,
        text_query: &str,
        word: &OcrWord,
        _content_left: u32,
        _content_top: u32,
        _content_width: u32,
        _content_height: u32,
    ) -> Option<OcrTextMatch> {
        let text = word.Text().ok()?.to_string();

        // Pre-filter (same normalization + fuzzy as macOS / Linux)
        let nq = normalize_for_match(text_query);
        let nt = normalize_for_match(&text);
        if !nt.contains(&nq) && !fuzzy_text_matches_query(&text, text_query) {
            return None;
        }

        // Windows OCR returns bounding rect in pixels, top-left origin, within the image
        let rect = word.BoundingRect().ok()?;
        let local_left = f64::from(rect.X);
        let local_top = f64::from(rect.Y);
        let width = f64::from(rect.Width);
        let height = f64::from(rect.Height);

        image_box_to_global_match(shot, text, 0.8, local_left, local_top, width, height)
    }
}

// ---------------------------------------------------------------------------
// Linux: Tesseract OCR via leptess bindings
// ---------------------------------------------------------------------------
#[cfg(target_os = "linux")]
mod linux_backend {
    use super::{
        filter_and_rank, fuzzy_text_matches_query, image_box_to_global_match,
        image_content_rect_or_full, normalize_for_match, OcrTextMatch,
    };
    use leptess::capi::TessPageIteratorLevel_RIL_WORD;
    use leptess::{leptonica, tesseract::TessApi};
    use openbitfun_core::agentic::tools::computer_use_host::ComputerScreenshot;
    use openbitfun_core::util::errors::{OpenBitFunError, OpenBitFunResult};

    pub(super) fn find_text_matches(
        shot: &ComputerScreenshot,
        text_query: &str,
    ) -> OpenBitFunResult<Vec<OcrTextMatch>> {
        let (content_left, content_top, content_width, content_height) =
            image_content_rect_or_full(shot);
        if content_width == 0 || content_height == 0 {
            return Err(OpenBitFunError::tool(
                "Screenshot content rect is empty; cannot run Linux Tesseract OCR.".to_string(),
            ));
        }

        // Initialize Tesseract API
        // Try system default tessdata path first, then common locations
        let mut api = match TessApi::new(None, "eng") {
            Ok(api) => api,
            Err(_) => {
                let paths = [
                    "/usr/share/tesseract-ocr/5/tessdata/",
                    "/usr/share/tesseract-ocr/tessdata/",
                    "/usr/share/tessdata/",
                ];
                let mut api = None;
                for path in &paths {
                    if std::path::Path::new(path).exists() {
                        if let Ok(a) = TessApi::new(Some(path), "eng") {
                            api = Some(a);
                            break;
                        }
                    }
                }
                api.ok_or_else(|| OpenBitFunError::tool(
                    "Linux OCR: Tesseract initialization failed. Please install tesseract-ocr and tesseract-ocr-eng packages, or ensure TESSDATA_PREFIX is set correctly.".to_string()
                ))?
            }
        };

        let pix = leptonica::pix_read_mem(&shot.bytes).map_err(|e| {
            OpenBitFunError::tool(format!(
                "Linux OCR: Failed to decode screenshot image with Leptonica: {}",
                e
            ))
        })?;

        api.set_image(&pix);
        if api.recognize() != 0 {
            return Err(OpenBitFunError::tool(
                "Linux OCR: Tesseract recognition failed.".to_string(),
            ));
        }

        let boxa = api
            .get_component_images(TessPageIteratorLevel_RIL_WORD, true)
            .ok_or_else(|| {
                OpenBitFunError::tool(
                    "Linux OCR: Tesseract did not return word regions.".to_string(),
                )
            })?;

        let word_region_count = boxa.get_n();
        let mut raw_matches = Vec::new();

        for b in &boxa {
            let g = b.get_geometry();
            if g.w <= 0 || g.h <= 0 {
                continue;
            }
            let x1 = g.x;
            let y1 = g.y;
            let x2 = g.x + g.w;
            let y2 = g.y + g.h;
            api.set_rectangle(g.x, g.y, g.w, g.h);
            let text = match api.get_utf8_text() {
                Ok(t) => t,
                Err(_) => continue,
            };
            let confidence = api.mean_text_conf() as f32 / 100.0;
            if let Some(m) = tesseract_word_to_match(
                shot,
                text_query,
                &text,
                confidence,
                x1,
                y1,
                x2,
                y2,
                content_left,
                content_top,
                content_width,
                content_height,
            ) {
                raw_matches.push(m);
            }
        }

        let ranked = filter_and_rank(text_query, raw_matches);
        if ranked.is_empty() {
            return Err(OpenBitFunError::tool(format!(
                "No OCR text matched {:?} on screen (Tesseract found {} word regions total).",
                text_query, word_region_count
            )));
        }
        Ok(ranked)
    }

    fn tesseract_word_to_match(
        shot: &ComputerScreenshot,
        text_query: &str,
        text: &str,
        confidence: f32,
        x1: i32,
        y1: i32,
        x2: i32,
        y2: i32,
        _content_left: u32,
        _content_top: u32,
        _content_width: u32,
        _content_height: u32,
    ) -> Option<OcrTextMatch> {
        let nq = normalize_for_match(text_query);
        let nt = normalize_for_match(text);
        if !nt.contains(&nq) && !fuzzy_text_matches_query(text, text_query) {
            return None;
        }

        // Tesseract returns bounding box in pixels, top-left origin, within the image
        let local_left = x1 as f64;
        let local_top = y1 as f64;
        let width = (x2 - x1) as f64;
        let height = (y2 - y1) as f64;

        if width <= 0.0 || height <= 0.0 {
            return None;
        }

        image_box_to_global_match(
            shot,
            text.to_string(),
            confidence,
            local_left,
            local_top,
            width,
            height,
        )
    }
}
