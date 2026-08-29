import React from 'react';

import type { FlowToolItem } from '../types/flow-chat';
import { PlanDisplay, type PlanDisplayProps } from './CreatePlanDisplay';

interface WritePlanDisplayProps {
  toolItem: FlowToolItem;
  planFilePath: string;
  initialContent: string;
  workspacePath?: string;
  remoteConnectionId?: string;
}

export const WritePlanDisplay: React.FC<WritePlanDisplayProps> = ({
  toolItem,
  planFilePath,
  initialContent,
  workspacePath,
  remoteConnectionId,
}) => (
  <PlanDisplay
    planFilePath={planFilePath}
    initialContent={initialContent}
    status={toolItem.status as PlanDisplayProps['status']}
    cacheKey={toolItem.id}
    toolName="Write"
    storageKind="project-file"
    workspacePath={workspacePath}
    remoteConnectionId={remoteConnectionId}
  />
);
