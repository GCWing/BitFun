pub(super) fn script() -> &'static str {
    r####"
    const releaseActions = async (pressedKeys, pressedButtons, frameContext = currentFrameContext) => {
      const runtime = ensureRuntimeState();
      for (const rawKey of pressedKeys || []) {
        const target = getActiveTarget(frameContext);
        const key = normalizeKeyValue(rawKey);
        dispatchKeyboardEvent(target, "keyup", key, runtime.modifiers, frameContext);
        updateModifierState(runtime.modifiers, key, false);
      }

      for (const item of pressedButtons || []) {
        const button = Number(item?.button || 0);
        const buttonMask = pointerButtonMask(button);
        const target =
          runtime.pointer.target ||
          updatePointerTarget(frameContext, runtime.pointer.x, runtime.pointer.y, getActiveTarget(frameContext));
        if (!target) {
          continue;
        }
        runtime.pointer.buttons &= ~buttonMask;
        dispatchMouseEvent(target, "mouseup", runtime.pointer.x, runtime.pointer.y, button, runtime.pointer.buttons, frameContext);
      }
    };
"####
}
