pub(super) fn script() -> &'static str {
    r####"
    const performKeySourceActions = async (keyState, source, frameContext = currentFrameContext) => {
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
    };
"####
}
