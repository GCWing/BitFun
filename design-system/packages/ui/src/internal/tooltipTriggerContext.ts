import { createContext } from "react";

/** An explicit tooltip owns this subtree; text slots should not open another one. */
export const TooltipTriggerContext = createContext(false);
