/**
 * The composer's permission control and a dispatch job's approval policy are
 * one user decision expressed on two sides of the transport.
 *
 * Both directions live here so they cannot drift: a dispatch session is
 * created with the policy the local permission default implies, and the
 * composer strip renders that policy back as the same control a local session
 * shows. There is deliberately no separate place to choose it — protocol v4
 * carries the policy per turn, so the composer is the only editor.
 */

import type { ChatInputPermissionMode } from '@/flow_chat/components/ChatInputWorkspaceStrip';
import type { SessionPermissionMode } from '@/infrastructure/api/service-api/AgentAPI';
import type { DispatchApprovalPolicy } from './types';

/** Control modes a dispatch session can express; `full_access` folds into `auto`. */
export type DispatchPermissionMode = Extract<
  ChatInputPermissionMode,
  'ask' | 'auto' | 'reject'
>;

/** The three modes the dispatch permission control offers. */
export const DISPATCH_PERMISSION_MODES: DispatchPermissionMode[] = ['ask', 'auto', 'reject'];

export function dispatchApprovalPolicyFromPermissionMode(
  mode: Exclude<ChatInputPermissionMode, 'acp'>,
): DispatchApprovalPolicy {
  // Full access already resolves every ask, so it reaches the target as the
  // policy that answers without a round trip.
  if (mode === 'auto' || mode === 'full_access') return 'auto';
  if (mode === 'reject') return 'reject-and-report';
  // `ask` keeps the user in the loop: the target forwards each request to this
  // controller's normal permission panel.
  return 'remote';
}

/**
 * The policy a new dispatch session starts with, taken from the same stored
 * default a local session in this workspace would resolve.
 */
export function dispatchApprovalPolicyFromSessionMode(
  mode: SessionPermissionMode,
): DispatchApprovalPolicy {
  return dispatchApprovalPolicyFromPermissionMode(mode === 'auto_approve' ? 'auto' : mode);
}

export function permissionModeFromDispatchApprovalPolicy(
  policy: DispatchApprovalPolicy | undefined,
): DispatchPermissionMode {
  if (policy === 'auto') return 'auto';
  if (policy === 'reject-and-report') return 'reject';
  return 'ask';
}
