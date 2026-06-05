pub(super) fn script() -> &'static str {
    r####"
    const ELEMENT_KEY = "element-6066-11e4-a52e-4f735466cecf";
    const SHADOW_KEY = "shadow-6066-11e4-a52e-4f735466cecf";
    const EVENT_NAME = "bitfun_webdriver_result";
    const STORE_KEY = "__bitfunWdElements";
    const LOG_KEY = "__bitfunWdLogs";
    const consolePatchedKey = "__bitfunWdConsolePatched";
    let currentFrameContext = [];

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

    const shouldIgnoreLogMessage = (message) => {
      return /^JSON error: missing field `cmd` at line 1 column \d+$/.test(message);
    };

    const setFrameContext = (frameContext) => {
      currentFrameContext = Array.isArray(frameContext) ? frameContext : [];
    };

    const getFrameContext = () => currentFrameContext;

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
            const message = args.map((item) => safeStringify(item)).join(" ");
            if (!shouldIgnoreLogMessage(message)) {
              ensureLogs().push({
                level: level === "warn" ? "WARNING" : level === "error" ? "SEVERE" : "INFO",
                message,
                timestamp: Date.now()
              });
            }
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
      const errors = [];
      const webviewPostMessage = window.chrome && window.chrome.webview
        && typeof window.chrome.webview.postMessage === "function"
        ? window.chrome.webview.postMessage.bind(window.chrome.webview)
        : null;
      const tauriInvoke = window.__TAURI__ && window.__TAURI__.core && typeof window.__TAURI__.core.invoke === "function"
        ? window.__TAURI__.core.invoke.bind(window.__TAURI__.core)
        : null;
      const internalInvoke = window.__TAURI_INTERNALS__ && typeof window.__TAURI_INTERNALS__.invoke === "function"
        ? window.__TAURI_INTERNALS__.invoke.bind(window.__TAURI_INTERNALS__)
        : null;

      if (webviewPostMessage) {
        try {
          webviewPostMessage(JSON.stringify(payload));
          return;
        } catch (error) {
          errors.push(`window.chrome.webview.postMessage failed: ${safeStringify(error)}`);
        }
      }

      if (tauriInvoke) {
        try {
          await tauriInvoke("webdriver_bridge_result", {
            request: { payload }
          });
          return;
        } catch (error) {
          errors.push(`core.invoke command failed: ${safeStringify(error)}`);
        }
      }

      if (window.__TAURI__ && window.__TAURI__.event && typeof window.__TAURI__.event.emit === "function") {
        try {
          await window.__TAURI__.event.emit(EVENT_NAME, payload);
          return;
        } catch (error) {
          errors.push(`window.__TAURI__.event.emit failed: ${safeStringify(error)}`);
        }
      }

      if (internalInvoke) {
        try {
          await internalInvoke("plugin:event|emit", {
            event: EVENT_NAME,
            payload
          });
          return;
        } catch (error) {
          errors.push(`__TAURI_INTERNALS__.invoke(plugin:event|emit) failed: ${safeStringify(error)}`);
        }

        try {
          await internalInvoke("webdriver_bridge_result", {
            request: { payload }
          });
          return;
        } catch (error) {
          errors.push(`__TAURI_INTERNALS__.invoke command failed: ${safeStringify(error)}`);
        }
      }

      throw new Error(
        errors.length > 0
          ? `Tauri bridge unavailable: ${errors.join("; ")}`
          : "Tauri bridge unavailable"
      );
    };
"####
}
