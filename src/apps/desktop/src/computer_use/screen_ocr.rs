use bitfun_core::agentic::tools::computer_use_host::{
    ComputerScreenshot, ComputerUseImageContentRect, OcrRegionNative,
};
use bitfun_core::infrastructure::try_get_path_manager_arc;
use bitfun_core::util::errors::{BitFunError, BitFunResult};
use image::codecs::jpeg::JpegEncoder;
use log::{info, warn};
use std::fs;
use std::io::Write;
use std::path::PathBuf;

use chrono::Utc;

#[derive(Debug, Clone)]
pub struct OcrTextMatch {
    pub text: String,
    pub confidence: f32,
    pub center_x: f64,
    pub center_y: f64,
    pub bounds_left: f64,
    pub bounds_top: f64,
    pub bounds_width: f64,
    pub bounds_height: f64,
}

pub fn find_text_matches(
    shot: &ComputerScreenshot,
    text_query: &str,
) -> BitFunResult<Vec<OcrTextMatch>> {
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
    Err(BitFunError::tool(
        "move_to_text OCR is not supported on this platform.".to_string(),
    ))
}

/// If unset or non-zero: write the exact JPEG passed to OCR into `computer_use_debug` under the app data dir (see implementation). Set `BITFUN_COMPUTER_USE_OCR_DEBUG=0` to disable.
fn ocr_debug_save_enabled() -> bool {
    match std::env::var("BITFUN_COMPUTER_USE_OCR_DEBUG") {
        Ok(v) if v == "0" || v.eq_ignore_ascii_case("false") => false,
        _ => true,
    }
}

/// Same directory as agent `screenshot` debug (`workspace/.bitfun/computer_use_debug`), when PathManager is available.
fn computer_use_ocr_debug_dir() -> PathBuf {
    if let Ok(pm) = try_get_path_manager_arc() {
        return pm
            .default_assistant_workspace_dir(None)
            .join(".bitfun")
            .join("computer_use_debug");
    }
    dirs::home_dir()
        .map(|h| {
            h.join(".bitfun")
                .join("personal_assistant")
                .join("workspace")
                .join(".bitfun")
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

fn normalize_query(text_query: &str) -> BitFunResult<String> {
    let q = text_query.trim();
    if q.is_empty() {
        return Err(BitFunError::tool(
            "move_to_text requires a non-empty text_query.".to_string(),
        ));
    }
    Ok(q.to_string())
}

fn normalize_for_match(s: &str) -> String {
    s.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

fn rank_matches(mut matches: Vec<OcrTextMatch>, query: &str) -> Vec<OcrTextMatch> {
    let normalized_query = normalize_for_match(query);
    matches.sort_by(|a, b| compare_match(a, b, &normalized_query));
    matches
}

fn compare_match(
    a: &OcrTextMatch,
    b: &OcrTextMatch,
    normalized_query: &str,
) -> std::cmp::Ordering {
    let sa = match_score(a, normalized_query);
    let sb = match_score(b, normalized_query);
    sb.cmp(&sa)
        .then_with(|| {
            b.confidence
                .partial_cmp(&a.confidence)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .then_with(|| {
            let da = normalized_len_delta(&a.text, normalized_query);
            let db = normalized_len_delta(&b.text, normalized_query);
            da.cmp(&db)
        })
}

fn match_score(m: &OcrTextMatch, normalized_query: &str) -> i32 {
    let text = normalize_for_match(&m.text);
    if text == normalized_query {
        4
    } else if text.starts_with(normalized_query) {
        3
    } else if text.contains(normalized_query) {
        2
    } else {
        1
    }
}

fn normalized_len_delta(text: &str, normalized_query: &str) -> usize {
    let l = normalize_for_match(text).chars().count();
    let q = normalized_query.chars().count();
    l.abs_diff(q)
}

fn filter_and_rank(query: &str, raw_matches: Vec<OcrTextMatch>) -> Vec<OcrTextMatch> {
    let normalized_query = normalize_for_match(query);
    let filtered = raw_matches
        .into_iter()
        .filter(|m| normalize_for_match(&m.text).contains(&normalized_query))
        .collect::<Vec<_>>();
    rank_matches(filtered, query)
}

fn image_content_rect_or_full(shot: &ComputerScreenshot) -> (u32, u32, u32, u32) {
    if let Some(rect) = &shot.image_content_rect {
        (rect.left, rect.top, rect.width, rect.height)
    } else {
        (0, 0, shot.image_width, shot.image_height)
    }
}

/// Map a rectangle in **full JPEG pixel space** (top-left origin) to global pointer coordinates.
/// Uses `image_content_rect`: only the inner content area maps linearly to `native_width` × `native_height`.
pub fn image_box_to_global_match(
    shot: &ComputerScreenshot,
    text: String,
    confidence: f32,
    local_left: f64,
    local_top: f64,
    width: f64,
    height: f64,
) -> OcrTextMatch {
    let (cl, ct, cw, ch) = image_content_rect_or_full(shot);
    let cw = cw as f64;
    let ch = ch as f64;
    let cl = cl as f64;
    let ct = ct as f64;
    let rel_x = local_left - cl;
    let rel_y = local_top - ct;
    let nw = shot.native_width as f64;
    let nh = shot.native_height as f64;
    let global_left = shot.display_origin_x as f64 + (rel_x / cw.max(1e-9)) * nw;
    let global_top = shot.display_origin_y as f64 + (rel_y / ch.max(1e-9)) * nh;
    let global_width = (width / cw.max(1e-9)) * nw;
    let global_height = (height / ch.max(1e-9)) * nh;
    let center_x = global_left + global_width / 2.0;
    let center_y = global_top + global_height / 2.0;
    OcrTextMatch {
        text,
        confidence,
        center_x,
        center_y,
        bounds_left: global_left,
        bounds_top: global_top,
        bounds_width: global_width,
        bounds_height: global_height,
    }
}

/// Crop the peek JPEG to a **global native** rectangle intersected with the capture, then rebuild
/// [`ComputerScreenshot`] so OCR and coordinate mapping stay consistent (full frame = content).
/// Unused while OCR uses raw capture only; kept for experiments against cropped JPEG workflows.
#[allow(dead_code)]
pub fn crop_shot_to_ocr_region(
    shot: ComputerScreenshot,
    region: &OcrRegionNative,
) -> BitFunResult<ComputerScreenshot> {
    if region.width == 0 || region.height == 0 {
        return Err(BitFunError::tool(
            "ocr_region_native width and height must be non-zero.".to_string(),
        ));
    }
    let (cl, ct, cw, ch) = image_content_rect_or_full(&shot);
    if cw == 0 || ch == 0 {
        return Err(BitFunError::tool(
            "Screenshot content rect is empty; cannot crop for OCR.".to_string(),
        ));
    }

    let ox = shot.display_origin_x as i64;
    let oy = shot.display_origin_y as i64;
    let nw = shot.native_width as i64;
    let nh = shot.native_height as i64;

    let rx0 = region.x0 as i64;
    let ry0 = region.y0 as i64;
    let rw = region.width as i64;
    let rh = region.height as i64;

    let ix0 = rx0.max(ox);
    let iy0 = ry0.max(oy);
    let ix1 = (rx0 + rw).min(ox + nw);
    let iy1 = (ry0 + rh).min(oy + nh);
    if ix1 <= ix0 || iy1 <= iy0 {
        return Err(BitFunError::tool(
            "ocr_region_native does not intersect the captured display. Check coordinates (global native pixels).".to_string(),
        ));
    }

    let jx0 = cl as f64 + ((ix0 - ox) as f64 / nw as f64) * cw as f64;
    let jy0 = ct as f64 + ((iy0 - oy) as f64 / nh as f64) * ch as f64;
    let jx1 = cl as f64 + ((ix1 - ox) as f64 / nw as f64) * cw as f64;
    let jy1 = ct as f64 + ((iy1 - oy) as f64 / nh as f64) * ch as f64;

    let px0 = jx0.floor().max(0.0) as u32;
    let py0 = jy0.floor().max(0.0) as u32;
    let px1 = jx1.ceil().min(shot.image_width as f64) as u32;
    let py1 = jy1.ceil().min(shot.image_height as f64) as u32;
    if px1 <= px0 || py1 <= py0 {
        return Err(BitFunError::tool(
            "OCR crop region is empty after mapping to image pixels.".to_string(),
        ));
    }

    let dyn_img = image::load_from_memory(&shot.bytes)
        .map_err(|e| BitFunError::tool(format!("OCR crop: decode JPEG: {}", e)))?;
    let rgb = dyn_img.to_rgb8();
    let img_w = rgb.width();
    let img_h = rgb.height();
    // Clamp to decoded dimensions (must match Vision / mapping; may differ from metadata by 1px).
    let px1 = px1.min(img_w);
    let py1 = py1.min(img_h);
    if px1 <= px0 || py1 <= py0 {
        return Err(BitFunError::tool(
            "OCR crop region is empty after clamping to decoded JPEG size.".to_string(),
        ));
    }
    let crop_w = px1 - px0;
    let crop_h = py1 - py0;
    if px0.saturating_add(crop_w) > img_w || py0.saturating_add(crop_h) > img_h {
        return Err(BitFunError::tool("OCR crop rectangle is out of image bounds.".to_string()));
    }
    let cropped_view = image::imageops::crop_imm(&rgb, px0, py0, crop_w, crop_h);
    let cropped = cropped_view.to_image();

    const OCR_CROP_JPEG_QUALITY: u8 = 75;
    let mut buf = Vec::new();
    let mut enc = JpegEncoder::new_with_quality(&mut buf, OCR_CROP_JPEG_QUALITY);
    enc
        .encode(
            cropped.as_raw(),
            cropped.width(),
            cropped.height(),
            image::ColorType::Rgb8,
        )
        .map_err(|e| BitFunError::tool(format!("OCR crop: encode JPEG: {}", e)))?;

    // Affine mapping must match `image_box_to_global_match`: global = origin + (jpeg_px - content_left) / content_w * native_capture.
    // Do **not** use ix0/ix1 (intersection clip) as display_origin/native size — they disagree with floor/ceil JPEG bounds px0..px1.
    let cl_f = cl as f64;
    let ct_f = ct as f64;
    let cw_f = cw as f64;
    let ch_f = ch as f64;
    let ox_f = shot.display_origin_x as f64;
    let oy_f = shot.display_origin_y as f64;
    let nw_f = shot.native_width as f64;
    let nh_f = shot.native_height as f64;
    let native_left = ox_f + (px0 as f64 - cl_f) / cw_f.max(1e-9) * nw_f;
    let native_top = oy_f + (py0 as f64 - ct_f) / ch_f.max(1e-9) * nh_f;
    let native_right = ox_f + (px1 as f64 - cl_f) / cw_f.max(1e-9) * nw_f;
    let native_bottom = oy_f + (py1 as f64 - ct_f) / ch_f.max(1e-9) * nh_f;
    let native_w = (native_right - native_left).round().max(1.0) as u32;
    let native_h = (native_bottom - native_top).round().max(1.0) as u32;

    Ok(ComputerScreenshot {
        bytes: buf,
        mime_type: "image/jpeg".to_string(),
        image_width: cropped.width(),
        image_height: cropped.height(),
        native_width: native_w,
        native_height: native_h,
        display_origin_x: native_left.round() as i32,
        display_origin_y: native_top.round() as i32,
        vision_scale: shot.vision_scale,
        pointer_image_x: None,
        pointer_image_y: None,
        screenshot_crop_center: None,
        point_crop_half_extent_native: None,
        navigation_native_rect: None,
        quadrant_navigation_click_ready: false,
        image_content_rect: Some(ComputerUseImageContentRect {
            left: 0,
            top: 0,
            width: cropped.width(),
            height: cropped.height(),
        }),
        som_labels: vec![],
        implicit_confirmation_crop_applied: false,
    })
}

// ---------------------------------------------------------------------------
// macOS: Vision framework OCR via objc2-vision
// ---------------------------------------------------------------------------
#[cfg(target_os = "macos")]
mod macos {
    use super::{
        filter_and_rank, image_box_to_global_match, image_content_rect_or_full, normalize_for_match,
        OcrTextMatch,
    };
    use bitfun_core::agentic::tools::computer_use_host::ComputerScreenshot;
    use bitfun_core::util::errors::{BitFunError, BitFunResult};
    use objc2::msg_send;
    use objc2::rc::Retained;
    use objc2::AnyThread;
    use objc2_foundation::{NSArray, NSData, NSDictionary, NSError, NSString};
    use objc2_vision::{
        VNImageOption, VNImageRectForNormalizedRect, VNImageRequestHandler,
        VNRecognizeTextRequest, VNRecognizeTextRequestRevision3, VNRecognizedTextObservation,
        VNRequest, VNRequestTextRecognitionLevel,
    };

    /// Top-N candidates per observation; Chinese matches often appear below rank 1.
    const TOP_CANDIDATES_MAX: usize = 10;

    pub fn find_text_matches(
        shot: &ComputerScreenshot,
        text_query: &str,
    ) -> BitFunResult<Vec<OcrTextMatch>> {
        let (_content_left, _content_top, content_width, content_height) =
            image_content_rect_or_full(shot);
        if content_width == 0 || content_height == 0 {
            return Err(BitFunError::tool(
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
            return Err(BitFunError::tool(format!(
                "No OCR text matched {:?} on screen (macOS Vision found {} text regions total). \
                 If the UI is Chinese, try a shorter substring (e.g. one or two characters) or ensure the text is visible in the capture; Vision may mis-read stylized UI.",
                text_query,
                observations.len()
            )));
        }
        Ok(ranked)
    }

    fn recognize_text_observations(
        jpeg_bytes: &[u8],
    ) -> BitFunResult<Vec<Retained<VNRecognizedTextObservation>>> {
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
            .map_err(ns_error_to_bitfun)?;

        // Collect results.
        let results = match request.results() {
            Some(arr) => arr,
            None => return Ok(Vec::new()),
        };
        Ok(results.to_vec())
    }

    fn ns_error_to_bitfun(err: Retained<NSError>) -> BitFunError {
        let desc = err.localizedDescription().to_string();
        BitFunError::tool(format!("macOS Vision OCR failed: {}", desc))
    }

    fn observation_to_match(
        shot: &ComputerScreenshot,
        text_query: &str,
        obs: &VNRecognizedTextObservation,
    ) -> Option<OcrTextMatch> {
        let candidates = obs.topCandidates(TOP_CANDIDATES_MAX as usize);
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

        Some(image_box_to_global_match(
            shot,
            text,
            chosen_confidence,
            local_left,
            local_top,
            width,
            height,
        ))
    }
}

// ---------------------------------------------------------------------------
// Windows: Windows.Media.Ocr UWP API
// ---------------------------------------------------------------------------
#[cfg(target_os = "windows")]
mod windows_backend {
    use super::{
        filter_and_rank, image_box_to_global_match, image_content_rect_or_full, normalize_for_match,
    };
    use bitfun_core::agentic::tools::computer_use_host::ComputerScreenshot;
    use bitfun_core::util::errors::{BitFunError, BitFunResult};
    use windows::core::HSTRING;
    use windows::Foundation::IAsyncOperation;
    use windows::Graphics::Imaging::{BitmapDecoder, SoftwareBitmap};
    use windows::Media::Ocr::{OcrEngine, OcrLine, OcrWord};
    use windows::Storage::Streams::{DataWriter, InMemoryRandomAccessStream};
    use windows::Win32::System::Com::{
        CoIncrementMTAUsage, CoInitializeEx, CoUninitialize, COINIT_APARTMENTTHREADED,
        COINIT_DISABLE_OLE1DDE,
    };

    pub fn find_text_matches(
        shot: &ComputerScreenshot,
        text_query: &str,
    ) -> BitFunResult<Vec<OcrTextMatch>> {
        let (content_left, content_top, content_width, content_height) =
            image_content_rect_or_full(shot);
        if content_width == 0 || content_height == 0 {
            return Err(BitFunError::tool(
                "Screenshot content rect is empty; cannot run Windows OCR.".to_string(),
            ));
        }

        // Initialize COM apartment for WinRT APIs
        // This must run on a thread initialized with COINIT_APARTMENTTHREADED
        // Windows.Media.Ocr requires STA thread
        let mut co_init = None;
        if let Err(_) = windows::Win32::System::Com::CoIncrementMTAUsage() {
            match unsafe {
                windows::Win32::System::Com::CoInitializeEx(
                    None,
                    windows::Win32::System::Com::COINIT_APARTMENTTHREADED
                        | windows::Win32::System::Com::COINIT_DISABLE_OLE1DDE,
                )
            } {
                Ok(_) => co_init = Some(()),
                Err(e) => {
                    return Err(BitFunError::tool(format!(
                        "Windows OCR COM initialization failed: {}",
                        e
                    )));
                }
            }
        }

        let result = (|| -> BitFunResult<Vec<OcrTextMatch>> {
            // 1. Write JPEG bytes to in-memory stream
            let stream = InMemoryRandomAccessStream::new()?;
            let writer = DataWriter::CreateDataWriter(&stream)?;
            writer.WriteBytes(&shot.bytes)?;
            writer.StoreAsync()?.get()?;
            writer.FlushAsync()?.get()?;
            writer.DetachStream()?;

            // 2. Decode JPEG to SoftwareBitmap
            let decoder = BitmapDecoder::CreateAsync(&stream)?.get()?;
            let software_bitmap = decoder.GetSoftwareBitmapAsync()?.get()?;

            // 3. Create OCR engine (use user profile languages)
            let engine = match OcrEngine::TryCreateFromUserProfileLanguages() {
                Ok(e) => e,
                Err(_) => {
                    // Fallback to English if user profile languages fail
                    let lang = windows::Globalization::Language::CreateLanguage(&HSTRING::from("en-US"))?;
                    if !OcrEngine::IsLanguageSupported(&lang)? {
                        return Err(BitFunError::tool(
                            "Windows OCR: No supported language packs installed.".to_string(),
                        ));
                    }
                    OcrEngine::TryCreateFromLanguage(&lang)?
                }
            };

            // 4. Run OCR recognition
            let ocr_result = engine.RecognizeAsync(&software_bitmap)?.get()?;
            let lines = ocr_result.Lines()?;

            let mut raw_matches = Vec::new();
            for line in lines {
                let words = line.Words()?;
                for word in words {
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
                return Err(BitFunError::tool(format!(
                    "No OCR text matched {:?} on screen (Windows OCR found {} text regions total).",
                    text_query,
                    lines.len()
                )));
            }
            Ok(ranked)
        })();

        // Uninitialize COM if we initialized it
        if co_init.is_some() {
            unsafe { windows::Win32::System::Com::CoUninitialize() };
        }

        result
    }

    fn ocr_word_to_match(
        shot: &ComputerScreenshot,
        text_query: &str,
        word: &OcrWord,
        content_left: u32,
        content_top: u32,
        content_width: u32,
        content_height: u32,
    ) -> Option<OcrTextMatch> {
        let text = word.Text().ok()?.to_string();

        // Pre-filter
        if !normalize_for_match(&text).contains(&normalize_for_match(text_query)) {
            return None;
        }

        // Windows OCR returns bounding rect in pixels, top-left origin, within the image
        let rect = word.BoundingRect().ok()?;
        let local_left = content_left as f64 + rect.X;
        let local_top = content_top as f64 + rect.Y;
        let width = rect.Width;
        let height = rect.Height;

        Some(image_box_to_global_match(
            shot,
            text,
            0.8,
            local_left,
            local_top,
            width,
            height,
        ))
    }
}

// ---------------------------------------------------------------------------
// Linux: Tesseract OCR via leptess bindings
// ---------------------------------------------------------------------------
#[cfg(target_os = "linux")]
mod linux_backend {
    use super::{
        filter_and_rank, image_box_to_global_match, image_content_rect_or_full, normalize_for_match,
    };
    use bitfun_core::agentic::tools::computer_use_host::ComputerScreenshot;
    use bitfun_core::util::errors::{BitFunError, BitFunResult};
    use leptess::{leptonica, tesseract::TessApi};
    use std::ffi::CString;

    pub fn find_text_matches(
        shot: &ComputerScreenshot,
        text_query: &str,
    ) -> BitFunResult<Vec<OcrTextMatch>> {
        let (content_left, content_top, content_width, content_height) =
            image_content_rect_or_full(shot);
        if content_width == 0 || content_height == 0 {
            return Err(BitFunError::tool(
                "Screenshot content rect is empty; cannot run Linux Tesseract OCR.".to_string(),
            ));
        }

        // Initialize Tesseract API
        // Try system default tessdata path first, then common locations
        let mut api = match TessApi::new(None, "eng") {
            Ok(api) => api,
            Err(_) => {
                // Try common tessdata paths
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
                api.ok_or_else(|| BitFunError::tool(
                    "Linux OCR: Tesseract initialization failed. Please install tesseract-ocr and tesseract-ocr-eng packages, or ensure TESSDATA_PREFIX is set correctly.".to_string()
                ))?
            }
        };

        // Load JPEG from bytes using Leptonica
        let pix = match unsafe {
            leptonica::pix_read_mem(shot.bytes.as_ptr() as *const u8, shot.bytes.len() as libc::size_t)
        } {
            Some(p) => p,
            None => {
                return Err(BitFunError::tool(
                    "Linux OCR: Failed to decode screenshot image with Leptonica.".to_string(),
                ));
            }
        };

        api.set_image(&pix);

        // Get iterator over recognized words
        let mut iter = match api.get_iterator() {
            Some(it) => it,
            None => {
                return Err(BitFunError::tool(
                    "Linux OCR: No text found in screenshot.".to_string(),
                ));
            }
        };

        let mut raw_matches = Vec::new();
        let level = leptess::tesseract::PageIteratorLevel::RIL_WORD;

        loop {
            // Get word text
            if let Some(text) = iter.get_utf8_text(level) {
                // Get bounding box
                if let Some((x1, y1, x2, y2)) = iter.bounding_box(level) {
                    if !text.trim().is_empty() {
                        let confidence = iter.confidence(level).unwrap_or(0.7) as f32 / 100.0;
                        if let Some(m) = tesseract_word_to_match(
                            shot,
                            text_query,
                            &text,
                            confidence,
                            x1, y1, x2, y2,
                            content_left,
                            content_top,
                            content_width,
                            content_height,
                        ) {
                            raw_matches.push(m);
                        }
                    }
                }
            }

            if !iter.next(level) {
                break;
            }
        }

        let ranked = filter_and_rank(text_query, raw_matches);
        if ranked.is_empty() {
            return Err(BitFunError::tool(format!(
                "No OCR text matched {:?} on screen (Tesseract found {} text regions total).",
                text_query,
                raw_matches.len()
            )));
        }
        Ok(ranked)
    }

    fn tesseract_word_to_match(
        shot: &ComputerScreenshot,
        text_query: &str,
        text: &str,
        confidence: f32,
        x1: i32, y1: i32, x2: i32, y2: i32,
        content_left: u32,
        content_top: u32,
        _content_width: u32,
        _content_height: u32,
    ) -> Option<OcrTextMatch> {
        // Pre-filter
        if !normalize_for_match(text).contains(&normalize_for_match(text_query)) {
            return None;
        }

        // Tesseract returns bounding box in pixels, top-left origin, within the image
        let local_left = content_left as f64 + x1 as f64;
        let local_top = content_top as f64 + y1 as f64;
        let width = (x2 - x1) as f64;
        let height = (y2 - y1) as f64;

        if width <= 0.0 || height <= 0.0 {
            return None;
        }

        Some(image_box_to_global_match(
            shot,
            text.to_string(),
            confidence,
            local_left,
            local_top,
            width,
            height,
        ))
    }
}
