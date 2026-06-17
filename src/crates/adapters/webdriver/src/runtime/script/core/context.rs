pub(super) fn script() -> &'static str {
    r####"
    const cssEscape = (value) => {
      if (typeof CSS !== "undefined" && typeof CSS.escape === "function") {
        return CSS.escape(String(value));
      }
      return String(value).replace(/[^a-zA-Z0-9_\u00A0-\uFFFF-]/g, (char) => `\\${char}`);
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

    const resolveRoot = (rootId, frameContext = currentFrameContext) => {
      if (!rootId) {
        return getCurrentDocument(frameContext);
      }
      return getElement(rootId) || getCurrentDocument(frameContext);
    };

    const sleep = (duration) =>
      new Promise((resolve) => setTimeout(resolve, Math.max(0, Number(duration) || 0)));

    const getActiveTarget = (frameContext = currentFrameContext) => {
      const doc = getCurrentDocument(frameContext);
      return doc.activeElement || doc.body || doc.documentElement;
    };

    const getOwnerWindow = (target, frameContext = currentFrameContext) =>
      target?.ownerDocument?.defaultView || getCurrentWindow(frameContext);
"####
}
