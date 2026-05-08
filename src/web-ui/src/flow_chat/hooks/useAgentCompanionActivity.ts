import { useEffect, useState } from 'react';
import {
  buildAgentCompanionActivity,
  subscribeAgentCompanionActivity,
  type AgentCompanionActivityPayload,
} from '../utils/agentCompanionActivity';

export function useAgentCompanionActivity(): AgentCompanionActivityPayload {
  const [activity, setActivity] = useState<AgentCompanionActivityPayload>(
    () => buildAgentCompanionActivity(),
  );

  useEffect(() => subscribeAgentCompanionActivity(setActivity), []);

  return activity;
}
