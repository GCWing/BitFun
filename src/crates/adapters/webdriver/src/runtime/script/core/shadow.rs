pub(super) fn script() -> &'static str {
    r####"
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
"####
}
