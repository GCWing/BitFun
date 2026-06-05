pub(super) fn script() -> &'static str {
    r####"
    const findByXpath = (root, xpath, frameContext = currentFrameContext) => {
      const results = [];
      const ownerDocument = root && root.ownerDocument ? root.ownerDocument : getCurrentDocument(frameContext);
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
          matches = findByXpath(root, value, frameContext);
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
"####
}
