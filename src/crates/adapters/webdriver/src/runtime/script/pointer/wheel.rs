pub(super) fn script() -> &'static str {
    r####"
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
"####
}
