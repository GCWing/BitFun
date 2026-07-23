pub(super) fn script() -> &'static str {
    r####"
    const performActions = async (sources, frameContext = currentFrameContext) => {
      const runtime = ensureRuntimeState();
      const keyState = {
        ctrl: !!runtime.modifiers.ctrl,
        shift: !!runtime.modifiers.shift,
        alt: !!runtime.modifiers.alt,
        meta: !!runtime.modifiers.meta
      };

      for (const source of sources || []) {
        if (!source || !Array.isArray(source.actions)) {
          continue;
        }

        if (source.type === "pointer") {
          await performPointerSourceActions(runtime, source, frameContext);
          continue;
        }

        if (source.type === "wheel") {
          await performWheelSourceActions(runtime, source, frameContext);
          continue;
        }

        if (source.type === "key") {
          await performKeySourceActions(keyState, source, frameContext);
        }
      }

      runtime.modifiers = keyState;
    };
"####
}
