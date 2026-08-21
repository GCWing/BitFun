/**
 * R-WF-18: data-source isolation between plain assistant Claws and
 * workflow-member Claws.
 *
 * The backend creates every assistant workspace under
 * `~/.bitfun/personal_assistant/workspace-<id>`:
 *  - plain assistants allocate a random 8-hex assistantId
 *    (service.rs generate_assistant_workspace_id);
 *  - workflow members use the semantic workflow node id
 *    (legion_control_tool.rs resolve_assistant_workspace_dir(Some(node.id))),
 *    e.g. `researcher`, `implementer`, `lint`.
 *
 * `isWorkflowClawWorkspace` keys on that shape difference so the independent
 * workflow-Claw list never mixes with the plain Claw list, and vice versa.
 * Naming follows the R-WF-15 convention: workflow word root, no legion.
 */

import type { WorkspaceInfo } from '@/shared/types';

/** Plain assistants allocate an 8-hex assistantId (service.rs). */
const PLAIN_ASSISTANT_ID_PATTERN = /^[0-9a-f]{8}$/;

/**
 * True when the assistant workspace belongs to a deployed workflow member
 * Claw (semantic node-id workspace, e.g. `workspace-researcher`).
 */
export function isWorkflowClawWorkspace(workspace: WorkspaceInfo | null | undefined): boolean {
  if (!workspace) {
    return false;
  }
  const assistantId = workspace.assistantId;
  if (!assistantId) {
    return false;
  }
  return !PLAIN_ASSISTANT_ID_PATTERN.test(assistantId);
}

/** True when the assistant workspace is a plain (non-workflow) Claw. */
export function isPlainAssistantWorkspace(workspace: WorkspaceInfo | null | undefined): boolean {
  return !isWorkflowClawWorkspace(workspace);
}

export interface AssistantWorkspacePartition {
  workflowClaws: WorkspaceInfo[];
  plainAssistants: WorkspaceInfo[];
}

/** Split the shared assistant workspace list without dropping either side. */
export function splitAssistantWorkspacesByWorkflow(
  workspaces: WorkspaceInfo[],
): AssistantWorkspacePartition {
  const workflowClaws: WorkspaceInfo[] = [];
  const plainAssistants: WorkspaceInfo[] = [];
  for (const workspace of workspaces) {
    if (isWorkflowClawWorkspace(workspace)) {
      workflowClaws.push(workspace);
    } else {
      plainAssistants.push(workspace);
    }
  }
  return { workflowClaws, plainAssistants };
}
