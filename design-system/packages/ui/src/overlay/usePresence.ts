import { useEffect, useState } from "react";

export type PresenceState = "entering" | "entered" | "exiting";

export interface PresenceSnapshot {
  present: boolean;
  state: PresenceState;
}

export function usePresence(open: boolean, exitDurationMs: number): PresenceSnapshot {
  const [present, setPresent] = useState(open);
  const [state, setState] = useState<PresenceState>(open ? "entered" : "exiting");

  useEffect(() => {
    if (open) {
      setPresent(true);
      setState("entering");
      const view = typeof window === "undefined" ? null : window;
      if (!view) {
        setState("entered");
        return;
      }
      let firstFrame = 0;
      let secondFrame = 0;
      firstFrame = view.requestAnimationFrame(() => {
        secondFrame = view.requestAnimationFrame(() => setState("entered"));
      });
      return () => {
        view.cancelAnimationFrame(firstFrame);
        view.cancelAnimationFrame(secondFrame);
      };
    }

    if (!present) return;
    setState("exiting");
    const view = typeof window === "undefined" ? null : window;
    if (!view) {
      setPresent(false);
      return;
    }
    const timer = view.setTimeout(() => setPresent(false), exitDurationMs);
    return () => view.clearTimeout(timer);
  }, [exitDurationMs, open, present]);

  return { present, state };
}
