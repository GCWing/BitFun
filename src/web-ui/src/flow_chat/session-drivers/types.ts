/**
 * SessionDriver — the seam between flow-chat orchestration and a session's
 * transport family.
 *
 * A driver owns everything flavor-specific about a session: how it is
 * created, deleted, renamed, readied, submitted to, and cancelled. Shared
 * choreography (optimistic turns, queues, title generation, store plumbing)
 * stays in flow-chat-manager and calls into the driver at the points where
 * flavors diverge. New chat features land for every flavor by adding one
 * driver member instead of branching in eight files.
 *
 * Naming note: this is deliberately not called "backend" — the codebase uses
 * "backend session" for the Rust-side agent session (`ensureBackendSession`).
 */

import type { FlowChatContext, SessionConfig } from '../services/flow-chat-manager/types';
import type { SessionTitleDescriptor } from '../utils/sessionTitle';
import type { SessionDriverId } from './resolve';

export type { SessionDriverId } from './resolve';

/**
 * Everything flavor-independent that session creation resolved before the
 * driver takes over: workspace identity, agent type, and the initial title.
 */
export interface SessionCreationSeed {
  config: SessionConfig;
  agentType: string;
  sessionName: string;
  titleDescriptor: SessionTitleDescriptor;
  workspacePath: string;
  projectWorkspacePath: string;
  workspaceId?: string;
  remoteConnectionId?: string;
  remoteSshHost?: string;
}

/** Cascade facts computed once by the shared caller before removal. */
export interface SessionCascadeRemoval {
  removedSessionIds: string[];
  removedActiveSession: boolean;
}

export interface SessionDriver {
  readonly id: SessionDriverId;

  /** Create the flavor's session and return its id. */
  createSession(context: FlowChatContext, seed: SessionCreationSeed): Promise<string>;

  /** Remove the session (and its cascade) from this controller. */
  deleteSession(
    context: FlowChatContext,
    sessionId: string,
    removal: SessionCascadeRemoval,
  ): Promise<void>;

  /** Archive the session; observer flavors treat this as a local dismiss. */
  archiveSession(
    context: FlowChatContext,
    sessionId: string,
    removal: SessionCascadeRemoval,
  ): Promise<void>;

  /** Rename and return the effective title. */
  renameSession(
    context: FlowChatContext,
    sessionId: string,
    title: string,
  ): Promise<string>;

  /** Make the session able to accept a submission (backend session, hydration). */
  ensureReady(context: FlowChatContext, sessionId: string): Promise<void>;

  /** Cancel the in-flight work for this session. Returns whether anything was cancelled. */
  cancel(context: FlowChatContext, sessionId: string): Promise<boolean>;
}
