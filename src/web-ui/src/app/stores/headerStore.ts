/**
 * headerStore — thin bridge that publishes active-session context to the UnifiedTopBar.
 *
 * ModernFlowChatContainer writes here whenever the active session changes.
 * UnifiedTopBar reads from here to render the unified context title and back button.
 *
 * Keeping this separate from overlayStore avoids a circular dependency between
 * the app shell (overlay) and the flow_chat module.
 */

import { create } from 'zustand';

export interface AssistantWorkspaceRef {
  id: string;
  rootPath: string;
}

export interface SessionHeaderContext {
  /** Session mode string, e.g. "Dispatcher", "Cowork", "Claw". */
  mode: string;
  /** Workspace root path shown next to the mode label. */
  workspacePath?: string;
  /** Resolved display name (same as sidebar), not the raw path basename. */
  workspaceDisplayName?: string;
  /** Assistant workspace used when creating a new Dispatcher session. */
  assistantWorkspace?: AssistantWorkspaceRef | null;
}

interface HeaderState {
  /** Active session context; null when no session is loaded. */
  sessionContext: SessionHeaderContext | null;

  /** Called by ModernFlowChatContainer when a session becomes active. */
  setSessionContext: (ctx: SessionHeaderContext) => void;

  /** Called when no session is active (e.g. app starts or session is closed). */
  clearSessionContext: () => void;
}

export const useHeaderStore = create<HeaderState>((set) => ({
  sessionContext: null,

  setSessionContext: (ctx) => set({ sessionContext: ctx }),

  clearSessionContext: () => set({ sessionContext: null }),
}));
