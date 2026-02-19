/**
 * Workflow system type definitions.
 * Compatible with Claude Agent Team / SubAgent architecture.
 */

export type AgentRole = 'orchestrator' | 'worker' | 'reviewer';

export type OrchestrationPattern =
  | 'single'
  | 'pipeline'
  | 'fan_out'
  | 'supervisor'
  | 'team';

export type TriggerType = 'manual' | 'slash_command' | 'hotkey';

export type WorkflowLocation = 'user' | 'project';

export interface WorkflowTrigger {
  type: TriggerType;
  command?: string;
  hotkey?: string;
}

export interface WorkflowIO {
  description: string;
  examples?: string[];
}

export interface AgentNodeConfig {
  name: string;
  description?: string;
  prompt: string;
  model: string;
  tools: string[];
  skills: string[];
  readonly: boolean;
  maxTurns?: number;
}

export interface AgentNode {
  id: string;
  role: AgentRole;
  agentRef?: string;
  inline?: AgentNodeConfig;
}

export interface OrchestrationConfig {
  pattern: OrchestrationPattern;
  supervisor?: {
    agentId: string;
    maxDelegationDepth: number;
  };
  steps?: Array<{
    agentId: string;
    condition?: string;
  }>;
}

export interface Workflow {
  id: string;
  name: string;
  displayName: string;
  description: string;
  icon: string;
  version: string;
  location: WorkflowLocation;
  enabled: boolean;
  tags: string[];
  trigger: WorkflowTrigger;
  input?: WorkflowIO;
  agents: AgentNode[];
  orchestration: OrchestrationConfig;
}

export type WorkflowEditorStep = 'basic' | 'agents' | 'orchestration' | 'preview';
