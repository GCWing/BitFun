pub(super) fn script() -> &'static str {
    r####"
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
"####
}
