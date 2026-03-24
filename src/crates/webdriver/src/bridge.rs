use std::sync::Arc;
use std::sync::OnceLock;
use std::time::Duration;

#[cfg(target_os = "macos")]
use block2::RcBlock;
#[cfg(target_os = "macos")]
use objc2::rc::Retained;
#[cfg(target_os = "macos")]
use objc2::runtime::AnyObject;
#[cfg(target_os = "macos")]
use objc2::MainThreadMarker;
#[cfg(target_os = "macos")]
use objc2_foundation::{NSDictionary, NSError, NSString};
#[cfg(target_os = "macos")]
use objc2_web_kit::{WKContentWorld, WKWebView};
use serde::Deserialize;
use serde_json::Value;
use tauri::AppHandle;
use tauri::Listener;
use tauri::Manager;
use tokio::sync::oneshot;

use crate::server::response::WebDriverErrorResponse;
use crate::server::AppState;
use crate::webdriver::FrameId;

const BRIDGE_EVENT: &str = "bitfun_webdriver_result";
static BRIDGE_STATE: OnceLock<Arc<AppState>> = OnceLock::new();

#[derive(Debug, Deserialize)]
pub struct BridgeResponse {
    #[serde(rename = "requestId")]
    request_id: String,
    ok: bool,
    value: Option<Value>,
    error: Option<BridgeError>,
}

#[derive(Debug, Deserialize)]
struct BridgeError {
    message: Option<String>,
    stack: Option<String>,
}

pub fn register_listener(app: AppHandle, state: Arc<AppState>) {
    let _ = BRIDGE_STATE.set(state.clone());
    app.listen_any(BRIDGE_EVENT, move |event| {
        let Ok(payload) = serde_json::from_str::<BridgeResponse>(event.payload()) else {
            return;
        };
        log::debug!(
            "Embedded WebDriver bridge received event payload: request_id={}, ok={}",
            payload.request_id,
            payload.ok
        );
        dispatch_bridge_response(&state, payload);
    });
}

pub fn handle_invoke_payload(payload: Value) -> Result<(), String> {
    let state = BRIDGE_STATE
        .get()
        .ok_or_else(|| "Embedded WebDriver bridge state is not initialized".to_string())?;
    let payload = serde_json::from_value::<BridgeResponse>(payload)
        .map_err(|error| format!("Invalid bridge payload: {error}"))?;
    log::debug!(
        "Embedded WebDriver bridge received invoke payload: request_id={}, ok={}",
        payload.request_id,
        payload.ok
    );
    dispatch_bridge_response(state, payload);
    Ok(())
}

fn dispatch_bridge_response(state: &Arc<AppState>, payload: BridgeResponse) {
    let maybe_sender = state
        .pending_requests
        .lock()
        .ok()
        .and_then(|mut pending| pending.remove(&payload.request_id));

    if let Some(sender) = maybe_sender {
        let _ = sender.send(payload);
    }
}

pub async fn run_script(
    state: Arc<AppState>,
    session_id: &str,
    script: &str,
    args: Vec<Value>,
    async_mode: bool,
) -> Result<Value, WebDriverErrorResponse> {
    let session = state.sessions.read().await.get_cloned(session_id)?;
    let timeout_ms = session.timeouts.script.max(5_000);
    let webview = state
        .app
        .get_webview(&session.current_window)
        .ok_or_else(|| {
            WebDriverErrorResponse::no_such_window(format!(
                "Webview not found: {}",
                session.current_window
            ))
        })?;

    let frame_context = serialize_frame_context(&session.frame_context);

    #[cfg(target_os = "macos")]
    {
        return run_script_native_macos(webview, timeout_ms, script, &args, async_mode, &frame_context)
            .await;
    }

    #[cfg(not(target_os = "macos"))]
    let request_id = state.next_request_id();
    #[cfg(not(target_os = "macos"))]
    let (sender, receiver) = oneshot::channel();

    #[cfg(not(target_os = "macos"))]
    state
        .pending_requests
        .lock()
        .map_err(|_| WebDriverErrorResponse::unknown_error("Failed to lock pending request map"))?
        .insert(request_id.clone(), sender);

    #[cfg(not(target_os = "macos"))]
    let injected = build_bridge_eval_script(&request_id, script, &args, async_mode, &frame_context);
    #[cfg(not(target_os = "macos"))]
    webview.eval(&injected).map_err(|error| {
        remove_pending_request(&state, &request_id);
        WebDriverErrorResponse::javascript_error(
            format!("Failed to evaluate script: {error}"),
            None,
        )
    })?;

    #[cfg(not(target_os = "macos"))]
    let response = tokio::time::timeout(Duration::from_millis(timeout_ms), receiver)
        .await
        .map_err(|_| {
            remove_pending_request(&state, &request_id);
            WebDriverErrorResponse::timeout(format!("Script timed out after {timeout_ms}ms"))
        })?
        .map_err(|_| {
            WebDriverErrorResponse::unknown_error("Bridge response channel closed unexpectedly")
        })?;

    #[cfg(not(target_os = "macos"))]
    {
        if response.ok {
            return Ok(response.value.unwrap_or(Value::Null));
        }

        let error = response.error.unwrap_or(BridgeError {
            message: Some("Unknown JavaScript error".into()),
            stack: None,
        });
        return Err(WebDriverErrorResponse::javascript_error(
            error
                .message
                .unwrap_or_else(|| "Unknown JavaScript error".into()),
            error.stack,
        ));
    }

    #[allow(unreachable_code)]
    Err(WebDriverErrorResponse::unknown_error(
        "Script execution is unavailable on this platform",
    ))
}

#[cfg(target_os = "macos")]
async fn run_script_native_macos<R: tauri::Runtime>(
    webview: tauri::Webview<R>,
    timeout_ms: u64,
    script: &str,
    args: &[Value],
    async_mode: bool,
    frame_context: &Value,
) -> Result<Value, WebDriverErrorResponse> {
    let wrapped = build_native_eval_script(script, args, async_mode, frame_context);
    let (sender, receiver) = oneshot::channel::<Result<String, String>>();

    let result = webview.with_webview(move |platform_webview| unsafe {
        let wk_webview: &WKWebView = &*platform_webview.inner().cast();
        let ns_script = NSString::from_str(&wrapped);
        let mtm = MainThreadMarker::new_unchecked();
        let empty_dict: Retained<NSDictionary<NSString, AnyObject>> = NSDictionary::new();
        let content_world = WKContentWorld::pageWorld(mtm);

        let sender = Arc::new(std::sync::Mutex::new(Some(sender)));
        let block = RcBlock::new(move |result: *mut AnyObject, error: *mut NSError| {
            let response = if !error.is_null() {
                Err((&*error).localizedDescription().to_string())
            } else if result.is_null() {
                Ok("null".to_string())
            } else {
                ns_object_to_string(&*result)
                    .ok_or_else(|| "Script returned a non-string payload".to_string())
            };

            if let Ok(mut guard) = sender.lock() {
                if let Some(sender) = guard.take() {
                    let _ = sender.send(response);
                }
            }
        });

        wk_webview.callAsyncJavaScript_arguments_inFrame_inContentWorld_completionHandler(
            &ns_script,
            Some(&empty_dict),
            None,
            &content_world,
            Some(&block),
        );
    });

    if let Err(error) = result {
        return Err(WebDriverErrorResponse::javascript_error(
            format!("Failed to evaluate script: {error}"),
            None,
        ));
    }

    let response_payload = tokio::time::timeout(Duration::from_millis(timeout_ms), receiver)
        .await
        .map_err(|_| WebDriverErrorResponse::timeout(format!("Script timed out after {timeout_ms}ms")))?
        .map_err(|_| {
            WebDriverErrorResponse::unknown_error("Script response channel closed unexpectedly")
        })?
        .map_err(|error| WebDriverErrorResponse::javascript_error(error, None))?;

    let response: BridgeResponse = serde_json::from_str(&response_payload).map_err(|error| {
        WebDriverErrorResponse::unknown_error(format!("Invalid native script response: {error}"))
    })?;

    if response.ok {
        return Ok(response.value.unwrap_or(Value::Null));
    }

    let error = response.error.unwrap_or(BridgeError {
        message: Some("Unknown JavaScript error".into()),
        stack: None,
    });
    Err(WebDriverErrorResponse::javascript_error(
        error
            .message
            .unwrap_or_else(|| "Unknown JavaScript error".into()),
        error.stack,
    ))
}

#[cfg(target_os = "macos")]
unsafe fn ns_object_to_string(obj: &AnyObject) -> Option<String> {
    let class_name = obj.class().name().to_str().unwrap_or("");
    if !class_name.contains("String") {
        return None;
    }

    let ns_string: &NSString = &*std::ptr::from_ref::<AnyObject>(obj).cast::<NSString>();
    Some(ns_string.to_string())
}

#[cfg(not(target_os = "macos"))]
fn remove_pending_request(state: &AppState, request_id: &str) {
    if let Ok(mut pending) = state.pending_requests.lock() {
        pending.remove(request_id);
    }
}

fn serialize_frame_context(frame_context: &[FrameId]) -> Value {
    Value::Array(
        frame_context
            .iter()
            .map(|frame_id| match frame_id {
                FrameId::Index(index) => serde_json::json!({
                    "kind": "index",
                    "value": index
                }),
                FrameId::Element(element_id) => serde_json::json!({
                    "kind": "element",
                    "value": element_id
                }),
            })
            .collect(),
    )
}

#[cfg(not(target_os = "macos"))]
fn build_bridge_eval_script(
    request_id: &str,
    script: &str,
    args: &[Value],
    async_mode: bool,
    frame_context: &Value,
) -> String {
    let request_id_json =
        serde_json::to_string(request_id).unwrap_or_else(|_| "\"invalid-request\"".into());
    let script_json = serde_json::to_string(script).unwrap_or_else(|_| "\"\"".into());
    let args_json = serde_json::to_string(args).unwrap_or_else(|_| "[]".into());
    let async_json = if async_mode { "true" } else { "false" };
    let frame_context_json = serde_json::to_string(frame_context).unwrap_or_else(|_| "[]".into());

    format!(
        r#"
(() => {{
  {helper}
  window.__bitfunWd.run({request_id}, {script}, {args}, {async_mode}, {frame_context});
}})();
"#,
        helper = bridge_helper_script(),
        request_id = request_id_json,
        script = script_json,
        args = args_json,
        async_mode = async_json,
        frame_context = frame_context_json
    )
}

#[cfg(target_os = "macos")]
fn build_native_eval_script(
    script: &str,
    args: &[Value],
    async_mode: bool,
    frame_context: &Value,
) -> String {
    let script_json = serde_json::to_string(script).unwrap_or_else(|_| "\"\"".into());
    let args_json = serde_json::to_string(args).unwrap_or_else(|_| "[]".into());
    let async_json = if async_mode { "true" } else { "false" };
    let frame_context_json = serde_json::to_string(frame_context).unwrap_or_else(|_| "[]".into());

    format!(
        r#"
return (async () => {{
  {helper}
  const response = await window.__bitfunWd.execute({script}, {args}, {async_mode}, {frame_context});
  return JSON.stringify({{
    requestId: "__native__",
    ok: response.ok,
    value: response.value,
    error: response.error ?? null
  }});
}})();
"#,
        helper = bridge_helper_script(),
        script = script_json,
        args = args_json,
        async_mode = async_json,
        frame_context = frame_context_json
    )
}

fn bridge_helper_script() -> &'static str {
    r#"
if (!window.__bitfunWd) {
  window.__bitfunWd = (() => {
    const ELEMENT_KEY = "element-6066-11e4-a52e-4f735466cecf";
    const SHADOW_KEY = "shadow-6066-11e4-a52e-4f735466cecf";
    const EVENT_NAME = "bitfun_webdriver_result";
    const STORE_KEY = "__bitfunWdElements";
    const LOG_KEY = "__bitfunWdLogs";
    const consolePatchedKey = "__bitfunWdConsolePatched";
    let currentFrameContext = [];

    const ensureStore = () => {
      if (!window[STORE_KEY]) {
        window[STORE_KEY] = Object.create(null);
      }
      return window[STORE_KEY];
    };

    const ensureLogs = () => {
      if (!window[LOG_KEY]) {
        window[LOG_KEY] = [];
      }
      return window[LOG_KEY];
    };

    const safeStringify = (value) => {
      if (typeof value === "string") {
        return value;
      }
      try {
        return JSON.stringify(value);
      } catch (_error) {
        return String(value);
      }
    };

    const cssEscape = (value) => {
      if (typeof CSS !== "undefined" && typeof CSS.escape === "function") {
        return CSS.escape(String(value));
      }
      return String(value).replace(/[^a-zA-Z0-9_\u00A0-\uFFFF-]/g, (char) => `\\${char}`);
    };

    const setFrameContext = (frameContext) => {
      currentFrameContext = Array.isArray(frameContext) ? frameContext : [];
    };

    const getFrameContext = () => currentFrameContext;

    const W3C_KEY_MAP = {
      "\uE000": "Unidentified",
      "\uE001": "Cancel",
      "\uE002": "Help",
      "\uE003": "Backspace",
      "\uE004": "Tab",
      "\uE005": "Clear",
      "\uE006": "Enter",
      "\uE007": "Enter",
      "\uE008": "Shift",
      "\uE009": "Control",
      "\uE00A": "Alt",
      "\uE00B": "Pause",
      "\uE00C": "Escape",
      "\uE00D": " ",
      "\uE00E": "PageUp",
      "\uE00F": "PageDown",
      "\uE010": "End",
      "\uE011": "Home",
      "\uE012": "ArrowLeft",
      "\uE013": "ArrowUp",
      "\uE014": "ArrowRight",
      "\uE015": "ArrowDown",
      "\uE016": "Insert",
      "\uE017": "Delete",
      "\uE031": "F1",
      "\uE032": "F2",
      "\uE033": "F3",
      "\uE034": "F4",
      "\uE035": "F5",
      "\uE036": "F6",
      "\uE037": "F7",
      "\uE038": "F8",
      "\uE039": "F9",
      "\uE03A": "F10",
      "\uE03B": "F11",
      "\uE03C": "F12",
      "\uE03D": "Meta"
    };

    const POINTER_BUTTON_MASK = {
      0: 1,
      1: 4,
      2: 2,
      3: 8,
      4: 16
    };

    const ensureRuntimeState = () => {
      if (!window.__bitfunWdRuntimeState) {
        window.__bitfunWdRuntimeState = {
          pointer: {
            x: 0,
            y: 0,
            target: null,
            buttons: 0,
            lastClickAt: 0,
            lastClickTargetId: null,
            lastClickButton: null
          },
          modifiers: {
            ctrl: false,
            shift: false,
            alt: false,
            meta: false
          }
        };
      }
      return window.__bitfunWdRuntimeState;
    };

    const patchConsole = () => {
      if (window[consolePatchedKey]) {
        return;
      }
      window[consolePatchedKey] = true;
      ["log", "info", "warn", "error", "debug"].forEach((level) => {
        const original = console[level];
        console[level] = (...args) => {
          try {
            ensureLogs().push({
              level: level === "warn" ? "WARNING" : level === "error" ? "SEVERE" : "INFO",
              message: args.map((item) => safeStringify(item)).join(" "),
              timestamp: Date.now()
            });
            if (ensureLogs().length > 200) {
              ensureLogs().splice(0, ensureLogs().length - 200);
            }
          } catch (_error) {}
          return original.apply(console, args);
        };
      });
    };

    const ensureAlertState = (targetWindow = window) => {
      if (!targetWindow.__bitfunWdAlertState) {
        targetWindow.__bitfunWdAlertState = {
          open: false,
          type: null,
          text: "",
          defaultValue: null,
          promptText: null
        };
      }
      return targetWindow.__bitfunWdAlertState;
    };

    const patchDialogs = (targetWindow = window) => {
      const patchedKey = "__bitfunWdDialogsPatched";
      if (targetWindow[patchedKey]) {
        return;
      }
      targetWindow[patchedKey] = true;

      const state = ensureAlertState(targetWindow);
      targetWindow.alert = (message) => {
        state.open = true;
        state.type = "alert";
        state.text = String(message ?? "");
        state.defaultValue = null;
        state.promptText = null;
      };
      targetWindow.confirm = (message) => {
        state.open = true;
        state.type = "confirm";
        state.text = String(message ?? "");
        state.defaultValue = null;
        state.promptText = null;
        return false;
      };
      targetWindow.prompt = (message, defaultValue = "") => {
        state.open = true;
        state.type = "prompt";
        state.text = String(message ?? "");
        state.defaultValue = defaultValue == null ? null : String(defaultValue);
        state.promptText = defaultValue == null ? null : String(defaultValue);
        return null;
      };
    };

    const emitResult = async (payload) => {
      if (window.__TAURI__ && window.__TAURI__.core && typeof window.__TAURI__.core.invoke === "function") {
        await window.__TAURI__.core.invoke("webdriver_bridge_result", {
          request: { payload }
        });
        return;
      }

      if (window.__TAURI__ && window.__TAURI__.event && typeof window.__TAURI__.event.emit === "function") {
        await window.__TAURI__.event.emit(EVENT_NAME, payload);
        return;
      }

      throw new Error("Tauri bridge unavailable");
    };

    const nextElementId = () => {
      window.__bitfunWdElementCounter = (window.__bitfunWdElementCounter || 0) + 1;
      return `bf-el-${window.__bitfunWdElementCounter}`;
    };

    const storeElement = (element) => {
      if (!element || typeof element !== "object") {
        return null;
      }
      const store = ensureStore();
      const existing = Object.entries(store).find(([, candidate]) => candidate === element);
      const id = existing ? existing[0] : nextElementId();
      store[id] = element;
      return { [ELEMENT_KEY]: id, ELEMENT: id };
    };

    const storeShadowRoot = (shadowRoot) => {
      if (!shadowRoot || typeof shadowRoot !== "object") {
        return null;
      }
      const store = ensureStore();
      const existing = Object.entries(store).find(([, candidate]) => candidate === shadowRoot);
      const id = existing ? existing[0] : nextElementId();
      store[id] = shadowRoot;
      return { [SHADOW_KEY]: id };
    };

    const getElement = (elementId) => {
      if (!elementId) {
        return null;
      }
      return ensureStore()[elementId] || null;
    };

    const isElementLike = (value) => !!value && typeof value === "object" && value.nodeType === 1;

    const getCurrentWindow = (frameContext = currentFrameContext) => {
      let currentWindowRef = window;
      for (const frameRef of frameContext || []) {
        let frameElement = null;
        if (!frameRef || typeof frameRef !== "object") {
          throw new Error("Invalid frame reference");
        }
        if (frameRef.kind === "index") {
          const frames = Array.from(currentWindowRef.document.querySelectorAll("iframe, frame"));
          frameElement = frames[Number(frameRef.value)];
        } else if (frameRef.kind === "element") {
          frameElement = getElement(String(frameRef.value));
        } else {
          throw new Error("Unsupported frame reference");
        }

        if (!frameElement || !isElementLike(frameElement)) {
          throw new Error("Unable to locate frame");
        }
        if (!/^(iframe|frame)$/i.test(String(frameElement.tagName || ""))) {
          throw new Error("Element is not a frame");
        }
        if (!frameElement.contentWindow) {
          throw new Error("Frame window is not available");
        }
        currentWindowRef = frameElement.contentWindow;
      }
      return currentWindowRef;
    };

    const getCurrentDocument = (frameContext = currentFrameContext) => {
      const currentWindowRef = getCurrentWindow(frameContext);
      if (!currentWindowRef.document) {
        throw new Error("Frame document is not available");
      }
      return currentWindowRef.document;
    };

    const serialize = (value, seen = new WeakSet()) => {
      if (value === undefined || value === null) {
        return value ?? null;
      }
      if (typeof value === "string" || typeof value === "number" || typeof value === "boolean") {
        return value;
      }
      if (isElementLike(value)) {
        return storeElement(value);
      }
      if (value && typeof value === "object" && typeof value.length === "number" && typeof value !== "string") {
        return Array.from(value).map((item) => serialize(item, seen));
      }
      if (value && typeof value === "object" && "x" in value && "y" in value && "width" in value && "height" in value && "top" in value && "left" in value) {
        return {
          x: value.x,
          y: value.y,
          width: value.width,
          height: value.height,
          top: value.top,
          right: value.right,
          bottom: value.bottom,
          left: value.left
        };
      }
      if (value && typeof value === "object" && "message" in value && "stack" in value) {
        return {
          name: value.name,
          message: value.message,
          stack: value.stack
        };
      }
      if (Array.isArray(value)) {
        return value.map((item) => serialize(item, seen));
      }
      if (typeof value === "object") {
        if (seen.has(value)) {
          return null;
        }
        seen.add(value);
        const out = {};
        Object.keys(value).forEach((key) => {
          out[key] = serialize(value[key], seen);
        });
        return out;
      }
      return String(value);
    };

    const deserialize = (value) => {
      if (Array.isArray(value)) {
        return value.map(deserialize);
      }
      if (value && typeof value === "object") {
        if (typeof value[ELEMENT_KEY] === "string") {
          return getElement(value[ELEMENT_KEY]);
        }
        if (typeof value[SHADOW_KEY] === "string") {
          return getElement(value[SHADOW_KEY]);
        }
        const out = {};
        Object.keys(value).forEach((key) => {
          out[key] = deserialize(value[key]);
        });
        return out;
      }
      return value;
    };

    const resolveRoot = (rootId, frameContext = currentFrameContext) => {
      if (!rootId) {
        return getCurrentDocument(frameContext);
      }
      return getElement(rootId) || getCurrentDocument(frameContext);
    };

    const findByXpath = (root, xpath) => {
      const results = [];
      const ownerDocument = root && root.ownerDocument ? root.ownerDocument : getCurrentDocument();
      const iterator = ownerDocument.evaluate(
        xpath,
        root,
        null,
        XPathResult.ORDERED_NODE_ITERATOR_TYPE,
        null
      );
      let node = iterator.iterateNext();
      while (node) {
        if (isElementLike(node)) {
          results.push(node);
        }
        node = iterator.iterateNext();
      }
      return results;
    };

    const findElements = (rootId, using, value, frameContext = currentFrameContext) => {
      const root = resolveRoot(rootId, frameContext);
      let matches = [];
      switch (using) {
        case "css selector":
          matches = Array.from(root.querySelectorAll(value));
          break;
        case "id":
          matches = Array.from(root.querySelectorAll(`#${cssEscape(value)}`));
          break;
        case "name":
          matches = Array.from(root.querySelectorAll(`[name="${cssEscape(value)}"]`));
          break;
        case "class name":
          matches = Array.from(root.getElementsByClassName(value));
          break;
        case "xpath":
          matches = findByXpath(root, value);
          break;
        case "link text":
          matches = Array.from(root.querySelectorAll("a")).filter((item) => (item.textContent || "").trim() === value);
          break;
        case "partial link text":
          matches = Array.from(root.querySelectorAll("a")).filter((item) => (item.textContent || "").includes(value));
          break;
        case "tag name":
          matches = Array.from(root.querySelectorAll(value));
          break;
        default:
          throw new Error(`Unsupported locator strategy: ${using}`);
      }
      return matches.map((item) => storeElement(item));
    };

    const validateFrameByIndex = (index, frameContext = currentFrameContext) => {
      const currentDocumentRef = getCurrentDocument(frameContext);
      const frames = Array.from(currentDocumentRef.querySelectorAll("iframe, frame"));
      return Number.isInteger(index) && index >= 0 && index < frames.length;
    };

    const validateFrameElement = (elementId) => {
      const element = getElement(elementId);
      return !!element && isElementLike(element) && /^(iframe|frame)$/i.test(String(element.tagName || "")) && !!element.contentWindow;
    };

    const getShadowRoot = (elementId) => {
      const element = getElement(elementId);
      if (!element || !isElementLike(element) || !element.shadowRoot) {
        return null;
      }
      return storeShadowRoot(element.shadowRoot);
    };

    const findElementsFromShadow = (shadowId, using, value, frameContext = currentFrameContext) => {
      const shadowRoot = getElement(shadowId);
      if (!shadowRoot) {
        throw new Error("No shadow root found");
      }
      return findElements(shadowId, using, value, frameContext);
    };

    const isDisplayed = (element) => {
      if (!element || !element.isConnected) {
        return false;
      }
      const style = window.getComputedStyle(element);
      if (style.display === "none" || style.visibility === "hidden" || style.visibility === "collapse") {
        return false;
      }
      if (Number(style.opacity || "1") === 0) {
        return false;
      }
      const rect = element.getBoundingClientRect();
      return rect.width > 0 && rect.height > 0;
    };

    const setSelectionRange = (element, start, end) => {
      if (typeof element.setSelectionRange === "function") {
        element.setSelectionRange(start, end);
      }
    };

    const emitInputEvents = (element) => {
      const ownerWindow = element?.ownerDocument?.defaultView || window;
      element.dispatchEvent(new ownerWindow.Event("input", { bubbles: true }));
      element.dispatchEvent(new ownerWindow.Event("change", { bubbles: true }));
    };

    const clearElement = (element) => {
      if (!element) {
        return;
      }
      if ("value" in element) {
        element.focus();
        element.value = "";
        emitInputEvents(element);
        return;
      }
      if (element.isContentEditable) {
        element.focus();
        element.textContent = "";
        emitInputEvents(element);
      }
    };

    const insertText = (element, text) => {
      if (!element) {
        return;
      }
      if ("value" in element) {
        const currentValue = String(element.value || "");
        const start = typeof element.selectionStart === "number" ? element.selectionStart : currentValue.length;
        const end = typeof element.selectionEnd === "number" ? element.selectionEnd : currentValue.length;
        const nextValue = currentValue.slice(0, start) + text + currentValue.slice(end);
        element.value = nextValue;
        const caret = start + text.length;
        setSelectionRange(element, caret, caret);
        emitInputEvents(element);
        return;
      }
      if (element.isContentEditable) {
        const ownerWindow = element.ownerDocument?.defaultView || window;
        const selection = ownerWindow.getSelection();
        element.focus();
        if (selection && selection.rangeCount > 0) {
          selection.deleteFromDocument();
          selection.getRangeAt(0).insertNode(element.ownerDocument.createTextNode(text));
          selection.collapseToEnd();
        } else {
          element.appendChild(element.ownerDocument.createTextNode(text));
        }
        emitInputEvents(element);
      }
    };

    const setElementText = (element, text) => {
      clearElement(element);
      insertText(element, text);
    };

    const sleep = (duration) =>
      new Promise((resolve) => setTimeout(resolve, Math.max(0, Number(duration) || 0)));

    const normalizeKeyValue = (value) => W3C_KEY_MAP[String(value)] || String(value || "");

    const isModifierKey = (key) =>
      key === "Control" || key === "Shift" || key === "Alt" || key === "Meta";

    const updateModifierState = (modifiers, key, isDown) => {
      if (key === "Control") modifiers.ctrl = isDown;
      if (key === "Shift") modifiers.shift = isDown;
      if (key === "Alt") modifiers.alt = isDown;
      if (key === "Meta") modifiers.meta = isDown;
    };

    const eventCodeForKey = (key) => {
      const specialCodes = {
        " ": "Space",
        Backspace: "Backspace",
        Tab: "Tab",
        Enter: "Enter",
        Escape: "Escape",
        Delete: "Delete",
        Insert: "Insert",
        Home: "Home",
        End: "End",
        PageUp: "PageUp",
        PageDown: "PageDown",
        ArrowLeft: "ArrowLeft",
        ArrowRight: "ArrowRight",
        ArrowUp: "ArrowUp",
        ArrowDown: "ArrowDown",
        Shift: "ShiftLeft",
        Control: "ControlLeft",
        Alt: "AltLeft",
        Meta: "MetaLeft"
      };
      if (specialCodes[key]) {
        return specialCodes[key];
      }
      if (/^F\d{1,2}$/.test(key)) {
        return key;
      }
      if (key.length === 1) {
        if (/^[a-z]$/i.test(key)) {
          return `Key${key.toUpperCase()}`;
        }
        if (/^\d$/.test(key)) {
          return `Digit${key}`;
        }
      }
      return key || "Unidentified";
    };

    const getPrintableKey = (key, modifiers) => {
      if (key === " ") {
        return " ";
      }
      if (key.length !== 1) {
        return key;
      }
      if (modifiers.shift && /^[a-z]$/.test(key)) {
        return key.toUpperCase();
      }
      return key;
    };

    const getActiveTarget = (frameContext = currentFrameContext) => {
      const doc = getCurrentDocument(frameContext);
      return doc.activeElement || doc.body || doc.documentElement;
    };

    const moveFocusByTab = (target, backwards, frameContext = currentFrameContext) => {
      const doc = getCurrentDocument(frameContext);
      const selector = [
        "a[href]",
        "button",
        "input",
        "select",
        "textarea",
        "[tabindex]:not([tabindex='-1'])"
      ].join(", ");
      const focusable = Array.from(doc.querySelectorAll(selector)).filter((element) => {
        if (!isElementLike(element) || element.disabled) {
          return false;
        }
        const style = window.getComputedStyle(element);
        return style.display !== "none" && style.visibility !== "hidden";
      });
      if (!focusable.length) {
        return;
      }
      const index = Math.max(0, focusable.indexOf(target));
      const nextIndex = backwards
        ? (index - 1 + focusable.length) % focusable.length
        : (index + 1) % focusable.length;
      focusable[nextIndex].focus();
    };

    const moveCaret = (target, direction) => {
      if (!target || !("value" in target)) {
        return;
      }
      const value = String(target.value || "");
      const start = typeof target.selectionStart === "number" ? target.selectionStart : value.length;
      const end = typeof target.selectionEnd === "number" ? target.selectionEnd : value.length;
      if (direction === "start") {
        setSelectionRange(target, 0, 0);
        return;
      }
      if (direction === "end") {
        setSelectionRange(target, value.length, value.length);
        return;
      }
      const base = direction === "left" ? Math.min(start, end) : Math.max(start, end);
      const next = direction === "left" ? Math.max(0, base - 1) : Math.min(value.length, base + 1);
      setSelectionRange(target, next, next);
    };

    const getOwnerWindow = (target, frameContext = currentFrameContext) =>
      target?.ownerDocument?.defaultView || getCurrentWindow(frameContext);

    const dispatchKeyboardEvent = (target, type, key, modifiers, frameContext = currentFrameContext) => {
      if (!target) {
        return true;
      }
      const ownerWindow = getOwnerWindow(target, frameContext);
      return target.dispatchEvent(
        new ownerWindow.KeyboardEvent(type, {
          key,
          code: eventCodeForKey(key),
          bubbles: true,
          cancelable: true,
          ctrlKey: modifiers.ctrl,
          shiftKey: modifiers.shift,
          altKey: modifiers.alt,
          metaKey: modifiers.meta
        })
      );
    };

    const applySpecialKey = (target, key, modifiers, frameContext = currentFrameContext) => {
      if (!target) {
        return;
      }

      const isInputLike = "value" in target;
      if ((modifiers.ctrl || modifiers.meta) && key.toLowerCase() === "a" && isInputLike) {
        const value = String(target.value || "");
        setSelectionRange(target, 0, value.length);
        return;
      }

      if (key === "Tab") {
        moveFocusByTab(target, modifiers.shift, frameContext);
        return;
      }

      if (key === "Backspace" && isInputLike) {
        const value = String(target.value || "");
        const start = typeof target.selectionStart === "number" ? target.selectionStart : value.length;
        const end = typeof target.selectionEnd === "number" ? target.selectionEnd : value.length;
        if (start !== end) {
          target.value = value.slice(0, start) + value.slice(end);
          setSelectionRange(target, start, start);
        } else if (start > 0) {
          target.value = value.slice(0, start - 1) + value.slice(end);
          setSelectionRange(target, start - 1, start - 1);
        }
        emitInputEvents(target);
        return;
      }

      if (key === "Delete" && isInputLike) {
        const value = String(target.value || "");
        const start = typeof target.selectionStart === "number" ? target.selectionStart : value.length;
        const end = typeof target.selectionEnd === "number" ? target.selectionEnd : value.length;
        if (start !== end) {
          target.value = value.slice(0, start) + value.slice(end);
        } else {
          target.value = value.slice(0, start) + value.slice(start + 1);
        }
        setSelectionRange(target, start, start);
        emitInputEvents(target);
        return;
      }

      if (key === "ArrowLeft" && isInputLike) {
        moveCaret(target, "left");
        return;
      }

      if (key === "ArrowRight" && isInputLike) {
        moveCaret(target, "right");
        return;
      }

      if (key === "Home" && isInputLike) {
        moveCaret(target, "start");
        return;
      }

      if (key === "End" && isInputLike) {
        moveCaret(target, "end");
        return;
      }

      if (key === "Enter") {
        if (isInputLike && String(target.tagName || "").toUpperCase() === "TEXTAREA" && !modifiers.ctrl && !modifiers.meta) {
          insertText(target, "\n");
        }
        return;
      }

      if (key.length === 1 && !modifiers.ctrl && !modifiers.meta && !modifiers.alt) {
        insertText(target, getPrintableKey(key, modifiers));
      }
    };

    const pointerButtonMask = (button) => POINTER_BUTTON_MASK[Number(button)] || 0;

    const getElementFromPoint = (frameContext, x, y) => {
      const doc = getCurrentDocument(frameContext);
      return doc.elementFromPoint(Number(x) || 0, Number(y) || 0);
    };

    const updatePointerTarget = (frameContext, x, y, fallbackTarget = null) => {
      const runtime = ensureRuntimeState();
      runtime.pointer.x = Number(x) || 0;
      runtime.pointer.y = Number(y) || 0;
      runtime.pointer.target =
        getElementFromPoint(frameContext, runtime.pointer.x, runtime.pointer.y) ||
        fallbackTarget ||
        runtime.pointer.target ||
        getActiveTarget(frameContext);
      return runtime.pointer.target;
    };

    const resolveActionOrigin = (origin, action, frameContext = currentFrameContext) => {
      const runtime = ensureRuntimeState();
      if (origin === "pointer") {
        return {
          x: runtime.pointer.x + (Number(action?.x) || 0),
          y: runtime.pointer.y + (Number(action?.y) || 0),
          target: null
        };
      }

      if (origin && typeof origin === "object" && typeof origin[ELEMENT_KEY] === "string") {
        const element = getElement(origin[ELEMENT_KEY]);
        if (!element) {
          throw new Error("Element not found");
        }
        const rect = element.getBoundingClientRect();
        return {
          x: rect.left + rect.width / 2 + (Number(action?.x) || 0),
          y: rect.top + rect.height / 2 + (Number(action?.y) || 0),
          target: element
        };
      }

      return {
        x: Number(action?.x) || 0,
        y: Number(action?.y) || 0,
        target: null
      };
    };

    const dispatchMouseEvent = (target, type, x, y, button, buttons, frameContext = currentFrameContext) => {
      if (!target) {
        return false;
      }
      const ownerWindow = getOwnerWindow(target, frameContext);
      return target.dispatchEvent(
        new ownerWindow.MouseEvent(type, {
          bubbles: true,
          cancelable: true,
          clientX: x,
          clientY: y,
          button,
          buttons
        })
      );
    };

    const maybeDispatchClick = (target, x, y, button, frameContext = currentFrameContext) => {
      if (!target) {
        return;
      }
      if (button === 2) {
        dispatchMouseEvent(target, "contextmenu", x, y, button, 0, frameContext);
        return;
      }
      dispatchMouseEvent(target, "click", x, y, button, 0, frameContext);
      const runtime = ensureRuntimeState();
      const clickTargetId = storeElement(target)?.[ELEMENT_KEY] || null;
      const now = Date.now();
      if (
        button === 0 &&
        runtime.pointer.lastClickButton === button &&
        runtime.pointer.lastClickTargetId === clickTargetId &&
        now - runtime.pointer.lastClickAt < 500
      ) {
        dispatchMouseEvent(target, "dblclick", x, y, button, 0, frameContext);
      }
      runtime.pointer.lastClickAt = now;
      runtime.pointer.lastClickButton = button;
      runtime.pointer.lastClickTargetId = clickTargetId;
    };

    const dispatchPointerClick = (element, button, doubleClick) => {
      if (!element) {
        throw new Error("Element not found");
      }
      element.scrollIntoView({ block: "center", inline: "center" });
      if (typeof element.focus === "function") {
        element.focus();
      }
      const rect = element.getBoundingClientRect();
      const x = rect.left + rect.width / 2;
      const y = rect.top + rect.height / 2;
      updatePointerTarget(getFrameContext(), x, y, element);
      const runtime = ensureRuntimeState();
      const buttonMask = pointerButtonMask(button);
      dispatchMouseEvent(element, "mouseover", x, y, button, runtime.pointer.buttons, getFrameContext());
      dispatchMouseEvent(element, "mousemove", x, y, button, runtime.pointer.buttons, getFrameContext());
      runtime.pointer.buttons |= buttonMask;
      dispatchMouseEvent(element, "mousedown", x, y, button, runtime.pointer.buttons, getFrameContext());
      runtime.pointer.buttons &= ~buttonMask;
      dispatchMouseEvent(element, "mouseup", x, y, button, runtime.pointer.buttons, getFrameContext());
      maybeDispatchClick(element, x, y, button, getFrameContext());
      if (doubleClick && button === 0) {
        dispatchMouseEvent(element, "dblclick", x, y, button, runtime.pointer.buttons, getFrameContext());
      }
    };

    const findScrollableTarget = (target, doc) => {
      let current = target;
      while (current && current !== doc.body && current !== doc.documentElement) {
        if (
          (current.scrollHeight > current.clientHeight || current.scrollWidth > current.clientWidth) &&
          current instanceof Element
        ) {
          return current;
        }
        current = current.parentElement;
      }
      return doc.scrollingElement || doc.documentElement || doc.body;
    };

    const applyWheelScroll = (target, deltaX, deltaY, frameContext = currentFrameContext) => {
      const doc = getCurrentDocument(frameContext);
      const scrollTarget = findScrollableTarget(target, doc);
      if (!scrollTarget) {
        return;
      }
      if (scrollTarget === doc.body || scrollTarget === doc.documentElement || scrollTarget === doc.scrollingElement) {
        const ownerWindow = doc.defaultView || window;
        ownerWindow.scrollBy(deltaX, deltaY);
        return;
      }
      scrollTarget.scrollLeft += deltaX;
      scrollTarget.scrollTop += deltaY;
    };

    const performActions = async (sources, frameContext = currentFrameContext) => {
      const runtime = ensureRuntimeState();
      const keyState = {
        ctrl: !!runtime.modifiers.ctrl,
        shift: !!runtime.modifiers.shift,
        alt: !!runtime.modifiers.alt,
        meta: !!runtime.modifiers.meta
      };

      for (const source of sources || []) {
        if (!source || !Array.isArray(source.actions)) {
          continue;
        }

        if (source.type === "pointer") {
          for (const action of source.actions) {
            if (action.type === "pause") {
              if (action.duration) {
                await sleep(action.duration);
              }
              continue;
            }

            if (action.type === "pointerMove") {
              if (action.duration) {
                await sleep(action.duration);
              }
              const origin = Object.prototype.hasOwnProperty.call(action, "origin") ? action.origin : "viewport";
              const resolved = resolveActionOrigin(origin, action, frameContext);
              const target = updatePointerTarget(frameContext, resolved.x, resolved.y, resolved.target);
              if (target) {
                dispatchMouseEvent(target, "mousemove", runtime.pointer.x, runtime.pointer.y, 0, runtime.pointer.buttons, frameContext);
              }
              continue;
            }

            const target =
              runtime.pointer.target ||
              updatePointerTarget(frameContext, runtime.pointer.x, runtime.pointer.y, getActiveTarget(frameContext));
            const button = Number(action.button || 0);
            const buttonMask = pointerButtonMask(button);
            if (!target) {
              throw new Error("Pointer target not found");
            }

            if (action.type === "pointerDown") {
              if (action.duration) {
                await sleep(action.duration);
              }
              if (typeof target.focus === "function") {
                target.focus();
              }
              runtime.pointer.buttons |= buttonMask;
              dispatchMouseEvent(target, "mousedown", runtime.pointer.x, runtime.pointer.y, button, runtime.pointer.buttons, frameContext);
              continue;
            }

            if (action.type === "pointerUp") {
              if (action.duration) {
                await sleep(action.duration);
              }
              runtime.pointer.buttons &= ~buttonMask;
              dispatchMouseEvent(target, "mouseup", runtime.pointer.x, runtime.pointer.y, button, runtime.pointer.buttons, frameContext);
              maybeDispatchClick(target, runtime.pointer.x, runtime.pointer.y, button, frameContext);
            }
          }
          continue;
        }

        if (source.type === "wheel") {
          for (const action of source.actions) {
            if (action.type === "pause") {
              if (action.duration) {
                await sleep(action.duration);
              }
              continue;
            }
            if (action.duration) {
              await sleep(action.duration);
            }
            const origin = Object.prototype.hasOwnProperty.call(action, "origin") ? action.origin : "viewport";
            const resolved = resolveActionOrigin(origin, action, frameContext);
            const target = updatePointerTarget(frameContext, resolved.x, resolved.y, resolved.target);
            if (target) {
              const ownerWindow = getOwnerWindow(target, frameContext);
              target.dispatchEvent(
                new ownerWindow.WheelEvent("wheel", {
                  bubbles: true,
                  cancelable: true,
                  clientX: runtime.pointer.x,
                  clientY: runtime.pointer.y,
                  deltaX: Number(action.deltaX) || 0,
                  deltaY: Number(action.deltaY) || 0
                })
              );
            }
            applyWheelScroll(target, Number(action.deltaX) || 0, Number(action.deltaY) || 0, frameContext);
          }
          continue;
        }

        if (source.type === "key") {
          for (const action of source.actions) {
            if (action.type === "pause") {
              if (action.duration) {
                await sleep(action.duration);
              }
              continue;
            }

            const target = getActiveTarget(frameContext);
            const key = normalizeKeyValue(action.value);
            if (action.type === "keyDown") {
              updateModifierState(keyState, key, true);
              dispatchKeyboardEvent(target, "keydown", key, keyState, frameContext);
              if (key.length === 1) {
                dispatchKeyboardEvent(target, "keypress", getPrintableKey(key, keyState), keyState, frameContext);
              }
              if (!isModifierKey(key)) {
                applySpecialKey(target, key, keyState, frameContext);
              }
              continue;
            }

            dispatchKeyboardEvent(target, "keyup", key, keyState, frameContext);
            updateModifierState(keyState, key, false);
          }
        }
      }

      runtime.modifiers = keyState;
    };

    const releaseActions = async (pressedKeys, pressedButtons, frameContext = currentFrameContext) => {
      const runtime = ensureRuntimeState();
      for (const rawKey of pressedKeys || []) {
        const target = getActiveTarget(frameContext);
        const key = normalizeKeyValue(rawKey);
        dispatchKeyboardEvent(target, "keyup", key, runtime.modifiers, frameContext);
        updateModifierState(runtime.modifiers, key, false);
      }

      for (const item of pressedButtons || []) {
        const button = Number(item?.button || 0);
        const buttonMask = pointerButtonMask(button);
        const target =
          runtime.pointer.target ||
          updatePointerTarget(frameContext, runtime.pointer.x, runtime.pointer.y, getActiveTarget(frameContext));
        if (!target) {
          continue;
        }
        runtime.pointer.buttons &= ~buttonMask;
        dispatchMouseEvent(target, "mouseup", runtime.pointer.x, runtime.pointer.y, button, runtime.pointer.buttons, frameContext);
      }
    };

    const parseDocumentCookies = (doc) => {
      const raw = doc.cookie || "";
      if (!raw.trim()) {
        return [];
      }
      return raw
        .split(/;\s*/)
        .filter(Boolean)
        .map((entry) => {
          const separator = entry.indexOf("=");
          const name = separator >= 0 ? entry.slice(0, separator) : entry;
          const value = separator >= 0 ? entry.slice(separator + 1) : "";
          return {
            name: decodeURIComponent(name),
            value: decodeURIComponent(value),
            path: null,
            domain: null,
            secure: false,
            httpOnly: false,
            expiry: null,
            sameSite: null
          };
        });
    };

    const getAllCookies = (frameContext = currentFrameContext) => parseDocumentCookies(getCurrentDocument(frameContext));

    const getCookie = (name, frameContext = currentFrameContext) =>
      getAllCookies(frameContext).find((cookie) => cookie.name === name) || null;

    const addCookie = (cookie, frameContext = currentFrameContext) => {
      if (!cookie || typeof cookie !== "object") {
        throw new Error("Invalid cookie payload");
      }
      if (!cookie.name) {
        throw new Error("Cookie name is required");
      }
      const doc = getCurrentDocument(frameContext);
      const parts = [
        `${encodeURIComponent(cookie.name)}=${encodeURIComponent(cookie.value ?? "")}`
      ];
      if (cookie.path) parts.push(`Path=${cookie.path}`);
      if (cookie.domain) parts.push(`Domain=${cookie.domain}`);
      if (cookie.expiry) parts.push(`Expires=${new Date(Number(cookie.expiry) * 1000).toUTCString()}`);
      if (cookie.secure) parts.push("Secure");
      if (cookie.sameSite) parts.push(`SameSite=${cookie.sameSite}`);
      doc.cookie = parts.join("; ");
      return null;
    };

    const deleteCookie = (name, frameContext = currentFrameContext) => {
      const doc = getCurrentDocument(frameContext);
      const expires = "Thu, 01 Jan 1970 00:00:00 GMT";
      doc.cookie = `${encodeURIComponent(name)}=; Expires=${expires}; Path=/`;
      doc.cookie = `${encodeURIComponent(name)}=; Expires=${expires}`;
      return null;
    };

    const deleteAllCookies = (frameContext = currentFrameContext) => {
      getAllCookies(frameContext).forEach((cookie) => {
        deleteCookie(cookie.name, frameContext);
      });
      return null;
    };

    const getAlertText = (frameContext = currentFrameContext) => {
      const targetWindow = getCurrentWindow(frameContext);
      const state = ensureAlertState(targetWindow);
      if (!state.open) {
        throw new Error("No alert is currently open");
      }
      return state.text || "";
    };

    const sendAlertText = (text, frameContext = currentFrameContext) => {
      const targetWindow = getCurrentWindow(frameContext);
      const state = ensureAlertState(targetWindow);
      if (!state.open) {
        throw new Error("No alert is currently open");
      }
      if (state.type !== "prompt") {
        throw new Error("Alert does not accept text");
      }
      state.promptText = text == null ? null : String(text);
      return null;
    };

    const closeAlert = (accepted, frameContext = currentFrameContext) => {
      const targetWindow = getCurrentWindow(frameContext);
      const state = ensureAlertState(targetWindow);
      if (!state.open) {
        throw new Error("No alert is currently open");
      }
      const result = {
        accepted: !!accepted,
        promptText: state.promptText
      };
      state.open = false;
      state.type = null;
      state.text = "";
      state.defaultValue = null;
      state.promptText = null;
      return result;
    };

    const toFunction = (script, targetWindow) => {
      const trimmed = String(script || "").trim();
      if (!trimmed) {
        return () => null;
      }

      try {
        return targetWindow.eval(`(${trimmed})`);
      } catch (_error) {
        return targetWindow.Function(trimmed);
      }
    };

    const execute = async (script, args, asyncMode, frameContext) => {
      patchConsole();
      try {
        setFrameContext(frameContext);
        const targetWindow = getCurrentWindow(frameContext);
        patchDialogs(targetWindow);
        const fn = toFunction(script, targetWindow);
        const resolvedArgs = deserialize(args);
        let value;
        if (asyncMode) {
          value = await new Promise((resolve, reject) => {
            const callback = (result) => resolve(result);
            try {
              fn.apply(targetWindow, [...resolvedArgs, callback]);
            } catch (error) {
              reject(error);
            }
          });
        } else {
          value = await fn.apply(targetWindow, resolvedArgs);
        }
        return {
          ok: true,
          value: serialize(value)
        };
      } catch (error) {
        return {
          ok: false,
          error: {
            name: error && error.name ? error.name : "Error",
            message: error && error.message ? error.message : String(error),
            stack: error && error.stack ? error.stack : null
          }
        };
      }
    };

    const run = async (requestId, script, args, asyncMode, frameContext) => {
      const response = await execute(script, args, asyncMode, frameContext);
      await emitResult({
        requestId,
        ok: response.ok,
        value: response.value,
        error: response.error
      });
    };

    const takeLogs = () => {
      const logs = ensureLogs().slice();
      ensureLogs().length = 0;
      return logs;
    };

    patchConsole();

    return {
      getElement,
      getCurrentWindow,
      getCurrentDocument,
      findElements,
      findElementsFromShadow,
      validateFrameByIndex,
      validateFrameElement,
      getShadowRoot,
      isDisplayed,
      clearElement,
      insertText,
      setElementText,
      dispatchPointerClick,
      performActions,
      releaseActions,
      getAllCookies,
      getCookie,
      addCookie,
      deleteCookie,
      deleteAllCookies,
      getAlertText,
      sendAlertText,
      closeAlert,
      takeLogs,
      execute,
      run
    };
  })();
}
"#
}
