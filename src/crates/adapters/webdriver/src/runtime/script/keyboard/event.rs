pub(super) fn script() -> &'static str {
    r####"
    const dispatchKeyboardEvent = (target, type, key, modifiers, frameContext = currentFrameContext) => {
      if (!target) {
        return true;
      }
      const ownerWindow = getOwnerWindow(target, frameContext);
      return target.dispatchEvent(
        new ownerWindow.KeyboardEvent(type, {
          key,
          code: eventCodeForKey(key),
          bubbles: true,
          cancelable: true,
          ctrlKey: modifiers.ctrl,
          shiftKey: modifiers.shift,
          altKey: modifiers.alt,
          metaKey: modifiers.meta
        })
      );
    };
"####
}
