pub(super) fn script() -> &'static str {
    r####"
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
"####
}
