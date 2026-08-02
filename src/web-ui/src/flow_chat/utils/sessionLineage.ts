import type { SessionLineageSnapshot } from '@/infrastructure/api/service-api/SessionAPI';
import type { SessionMetadata } from '@/shared/types/session-history';
import type { Session } from '../types/flow-chat';
import { deriveSessionRelationshipFromMetadata } from './sessionMetadata';

export type SessionLineageLifecycle =
  | 'running'
  | 'finishing'
  | 'waiting'
  | 'completed'
  | 'cancelled'
  | 'error'
  | 'idle';

export interface SessionLineageNode {
  sessionId: string;
  parentSessionId?: string;
  parentToolCallId?: string;
  title: string;
  agentType?: string;
  subagentType?: string;
  lifecycle: SessionLineageLifecycle;
  createdAt: number;
  workspacePath?: string;
  remoteConnectionId?: string;
  remoteSshHost?: string;
  isRoot: boolean;
  children: SessionLineageNode[];
}

type FlatSessionLineageNode = Omit<SessionLineageNode, 'children'>;

function metadataLifecycle(metadata: SessionMetadata): SessionLineageLifecycle {
  if (metadata.needsUserAttention) return 'waiting';
  if (metadata.unreadCompletion === 'error' || metadata.unreadCompletion === 'interrupted') {
    return 'error';
  }
  return metadata.status === 'completed' ? 'completed' : 'idle';
}

function sessionLifecycle(session: Session): SessionLineageLifecycle {
  if (session.needsUserAttention) return 'waiting';
  if (session.status === 'error' || session.hasUnreadCompletion === 'error') return 'error';

  const latestTurn = session.dialogTurns[session.dialogTurns.length - 1];
  switch (latestTurn?.status) {
    case 'pending':
    case 'image_analyzing':
    case 'processing':
      return 'running';
    case 'finishing':
    case 'cancelling':
      return 'finishing';
    case 'cancelled':
      return 'cancelled';
    case 'error':
      return 'error';
    case 'completed':
      return 'completed';
    default:
      return session.persistedStatus === 'completed' ? 'completed' : 'idle';
  }
}

function nodeFromMetadata(metadata: SessionMetadata): FlatSessionLineageNode {
  const relationship = deriveSessionRelationshipFromMetadata(metadata);
  return {
    sessionId: metadata.sessionId,
    parentSessionId: relationship.parentSessionId,
    parentToolCallId: relationship.parentToolCallId,
    title: metadata.sessionName,
    agentType: metadata.agentType,
    subagentType: relationship.subagentType,
    lifecycle: metadataLifecycle(metadata),
    createdAt: metadata.createdAt,
    workspacePath: metadata.workspacePath,
    remoteConnectionId: metadata.remoteConnectionId,
    remoteSshHost: metadata.remoteSshHost,
    isRoot: false,
  };
}

function nodeFromSession(session: Session): FlatSessionLineageNode {
  return {
    sessionId: session.sessionId,
    parentSessionId: session.parentSessionId,
    parentToolCallId: session.parentToolCallId,
    title: session.title?.trim() || session.subagentType || session.mode || 'Agent',
    agentType: session.mode || session.config.agentType,
    subagentType: session.subagentType,
    lifecycle: sessionLifecycle(session),
    createdAt: session.createdAt,
    workspacePath: session.workspacePath,
    remoteConnectionId: session.remoteConnectionId,
    remoteSshHost: session.remoteSshHost,
    isRoot: false,
  };
}

function resolveRootSessionId(
  nodes: Map<string, FlatSessionLineageNode>,
  anchorSessionId: string,
  snapshotRootSessionId?: string,
): string | null {
  if (snapshotRootSessionId && nodes.has(snapshotRootSessionId)) {
    return snapshotRootSessionId;
  }
  if (!nodes.has(anchorSessionId)) {
    return null;
  }

  let currentSessionId = anchorSessionId;
  const visited = new Set([currentSessionId]);
  while (true) {
    const parentSessionId = nodes.get(currentSessionId)?.parentSessionId;
    if (!parentSessionId || !nodes.has(parentSessionId) || visited.has(parentSessionId)) {
      return currentSessionId;
    }
    visited.add(parentSessionId);
    currentSessionId = parentSessionId;
  }
}

export function buildSessionLineageTree(
  anchorSessionId: string,
  snapshot: SessionLineageSnapshot | null,
  liveSessions: Map<string, Session>,
): SessionLineageNode | null {
  const nodes = new Map<string, FlatSessionLineageNode>();
  for (const metadata of snapshot?.sessions ?? []) {
    nodes.set(metadata.sessionId, nodeFromMetadata(metadata));
  }

  for (const session of liveSessions.values()) {
    if (
      session.sessionId === anchorSessionId ||
      session.sessionKind === 'subagent' ||
      nodes.has(session.sessionId)
    ) {
      nodes.set(session.sessionId, nodeFromSession(session));
    }
  }

  const rootSessionId = resolveRootSessionId(nodes, anchorSessionId, snapshot?.rootSessionId);
  if (!rootSessionId) return null;

  const childrenByParent = new Map<string, FlatSessionLineageNode[]>();
  for (const node of nodes.values()) {
    if (!node.parentSessionId || !nodes.has(node.parentSessionId)) continue;
    const children = childrenByParent.get(node.parentSessionId) ?? [];
    children.push(node);
    childrenByParent.set(node.parentSessionId, children);
  }
  for (const children of childrenByParent.values()) {
    children.sort((left, right) =>
      left.createdAt - right.createdAt || left.sessionId.localeCompare(right.sessionId)
    );
  }

  const visited = new Set<string>();
  const buildNode = (sessionId: string): SessionLineageNode | null => {
    if (visited.has(sessionId)) return null;
    const node = nodes.get(sessionId);
    if (!node) return null;
    visited.add(sessionId);
    return {
      ...node,
      isRoot: sessionId === rootSessionId,
      children: (childrenByParent.get(sessionId) ?? [])
        .map(child => buildNode(child.sessionId))
        .filter((child): child is SessionLineageNode => child !== null),
    };
  };

  return buildNode(rootSessionId);
}

export function countSessionLineageDescendants(root: SessionLineageNode | null): number {
  if (!root) return 0;
  return root.children.reduce(
    (count, child) => count + 1 + countSessionLineageDescendants(child),
    0,
  );
}

export function collectExpandedRunningBranches(root: SessionLineageNode | null): Set<string> {
  const expanded = new Set<string>();
  const visit = (node: SessionLineageNode): boolean => {
    const hasActiveDescendant = node.children.some(visit);
    const isActive = node.lifecycle === 'running' || node.lifecycle === 'finishing';
    if (node.isRoot || hasActiveDescendant) expanded.add(node.sessionId);
    return isActive || hasActiveDescendant;
  };
  if (root) visit(root);
  return expanded;
}
