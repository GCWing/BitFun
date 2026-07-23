pub(super) fn script() -> &'static str {
    r####"
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
"####
}
