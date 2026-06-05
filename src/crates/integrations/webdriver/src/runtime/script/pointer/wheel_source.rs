pub(super) fn script() -> &'static str {
    r####"
    const performWheelSourceActions = async (runtime, source, frameContext = currentFrameContext) => {
      for (const action of source.actions) {
        if (action.type === "pause") {
          if (action.duration) {
            await sleep(action.duration);
          }
          continue;
        }
        if (action.duration) {
          await sleep(action.duration);
        }
        const origin = Object.prototype.hasOwnProperty.call(action, "origin") ? action.origin : "viewport";
        const resolved = resolveActionOrigin(origin, action, frameContext);
        const target = updatePointerTarget(frameContext, resolved.x, resolved.y, resolved.target);
        if (target) {
          const ownerWindow = getOwnerWindow(target, frameContext);
          target.dispatchEvent(
            new ownerWindow.WheelEvent("wheel", {
              bubbles: true,
              cancelable: true,
              clientX: runtime.pointer.x,
              clientY: runtime.pointer.y,
              deltaX: Number(action.deltaX) || 0,
              deltaY: Number(action.deltaY) || 0,
              ctrlKey: !!runtime.modifiers.ctrl,
              shiftKey: !!runtime.modifiers.shift,
              altKey: !!runtime.modifiers.alt,
              metaKey: !!runtime.modifiers.meta
            })
          );
        }
        applyWheelScroll(target, Number(action.deltaX) || 0, Number(action.deltaY) || 0, frameContext);
      }
    };
"####
}
