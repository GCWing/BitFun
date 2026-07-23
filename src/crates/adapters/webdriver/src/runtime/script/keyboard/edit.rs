pub(super) fn script() -> &'static str {
    r####"
    const deleteSelectionOrPreviousChar = (target, value, start, end) => {
      if (start !== end) {
        setElementValue(target, value.slice(0, start) + value.slice(end));
        setSelectionRange(target, start, start);
        return;
      }
      if (start > 0) {
        setElementValue(target, value.slice(0, start - 1) + value.slice(end));
        setSelectionRange(target, start - 1, start - 1);
      }
    };

    const deleteSelectionOrNextChar = (target, value, start, end) => {
      if (start !== end) {
        setElementValue(target, value.slice(0, start) + value.slice(end));
      } else {
        setElementValue(target, value.slice(0, start) + value.slice(start + 1));
      }
      setSelectionRange(target, start, start);
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
        const value = String(getElementValue(target) || "");
        const start = typeof target.selectionStart === "number" ? target.selectionStart : value.length;
        const end = typeof target.selectionEnd === "number" ? target.selectionEnd : value.length;
        if (!dispatchBeforeInputEvent(target, "deleteContentBackward", null)) {
          return;
        }
        deleteSelectionOrPreviousChar(target, value, start, end);
        emitInputEvents(target, "deleteContentBackward", null);
        return;
      }

      if (key === "Delete" && isInputLike) {
        const value = String(getElementValue(target) || "");
        const start = typeof target.selectionStart === "number" ? target.selectionStart : value.length;
        const end = typeof target.selectionEnd === "number" ? target.selectionEnd : value.length;
        if (!dispatchBeforeInputEvent(target, "deleteContentForward", null)) {
          return;
        }
        deleteSelectionOrNextChar(target, value, start, end);
        emitInputEvents(target, "deleteContentForward", null);
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
"####
}
