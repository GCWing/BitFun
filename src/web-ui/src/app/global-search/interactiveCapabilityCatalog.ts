import type { SceneTabId } from '@/app/components/SceneBar/types';
import type { SettingsDestination } from '@/app/scenes/settings/settingsTypes';
import type { ProductActionId } from './productActionCatalog';
import generatedCatalog from './generated/interactive-capabilities.json';

export type InteractiveCapabilityKind = 'feature' | 'setting';
export type InteractiveCapabilityRisk = 'read' | 'write' | 'ui' | 'execute' | 'destructive';

export type InteractiveCapabilityDestination =
  | ({ kind: 'settings' } & SettingsDestination)
  | { kind: 'action'; actionId: ProductActionId }
  | { kind: 'scene'; sceneId: SceneTabId }
  | { kind: 'event'; eventName: string; detail?: Record<string, unknown> };

export interface InteractiveCapabilityValueSchema {
  type: 'boolean' | 'string' | 'integer' | 'number' | 'object' | 'array';
  nullable?: boolean;
  enum?: unknown[];
  minimum?: number;
  maximum?: number;
  minLength?: number;
  maxLength?: number;
}

export type InteractiveCapabilityOperationHandler =
  | { kind: 'productAction'; actionId: ProductActionId }
  | { kind: 'provider'; providerId: string; operationId: string };

export type InteractiveCapabilityOptionHandler =
  | { kind: 'config'; path: string }
  | { kind: 'mergeConfig'; path: string; fields: string[] }
  | { kind: 'appearanceSelection' }
  | { kind: 'language' }
  | { kind: 'provider'; providerId: string; optionId: string };

export interface InteractiveCapabilityOperation {
  id: string;
  titleZh: string;
  titleEn: string;
  descriptionZh: string;
  descriptionEn: string;
  risk: InteractiveCapabilityRisk;
  inputSchema: Record<string, unknown>;
  argumentScopes?: Record<string, 'productHostLocal'>;
  handler: InteractiveCapabilityOperationHandler;
}

export interface InteractiveCapabilityOption {
  id: string;
  titleZh: string;
  titleEn: string;
  descriptionZh: string;
  descriptionEn: string;
  valueSchema: InteractiveCapabilityValueSchema;
  handler: InteractiveCapabilityOptionHandler;
}

export interface InteractiveCapabilityItem {
  id: string;
  titleZh: string;
  titleEn: string;
  destination?: InteractiveCapabilityDestination;
  control:
    | {
      kind: 'direct';
      operations: string[];
      options: Array<{ id: string; value?: unknown }>;
    }
    | {
      kind: 'delegate';
      tools: string[];
      workflowZh: string[];
      workflowEn: string[];
    }
    | { kind: 'open' | 'unsupported'; reasonZh: string; reasonEn: string };
}

export interface InteractiveCapabilityAgentControl {
  tool: string;
  workflowZh: string[];
  workflowEn: string[];
}

export interface InteractiveCapability {
  id: string;
  kind: InteractiveCapabilityKind;
  titleZh: string;
  titleEn: string;
  summaryZh: string;
  summaryEn: string;
  categoryId: string;
  keywordsZh: string[];
  keywordsEn: string[];
  highlightsZh: string[];
  highlightsEn: string[];
  items: InteractiveCapabilityItem[];
  stepsZh: string[];
  stepsEn: string[];
  agentExamplesZh: string[];
  agentExamplesEn: string[];
  agentControl?: InteractiveCapabilityAgentControl;
  destination: InteractiveCapabilityDestination;
  operations: InteractiveCapabilityOperation[];
  options: InteractiveCapabilityOption[];
  searchTerms: string[];
  docsUrl: string;
}

export interface InteractiveCapabilityCatalog {
  schemaVersion: number;
  product: string;
  title: string;
  origin: string;
  source: string;
  digest: string;
  searchAcceptance: Array<{
    id: string;
    query: string;
    expectedFirstCapabilityId: string;
    expectedCapabilityIds: string[];
    expectedItem?: { capabilityId: string; itemId: string };
  }>;
  counts: {
    features: number;
    settings: number;
    userFacing: number;
    documentedItems: number;
    controlCoverage: {
      direct: number;
      delegated: number;
      interactive: number;
      unsupported: number;
    };
  };
  categories: Record<string, {
    titleZh: string;
    titleEn: string;
    descriptionZh: string;
    descriptionEn: string;
  }>;
  capabilities: InteractiveCapability[];
}

export const INTERACTIVE_CAPABILITY_CATALOG = generatedCatalog as InteractiveCapabilityCatalog;

const capabilityById = new Map(
  INTERACTIVE_CAPABILITY_CATALOG.capabilities.map((capability) => [capability.id, capability]),
);

export function getInteractiveCapability(capabilityId: string): InteractiveCapability | undefined {
  return capabilityById.get(capabilityId);
}
