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
  enum?: unknown[];
  minimum?: number;
  maximum?: number;
}

export type InteractiveCapabilityOperationHandler = {
  kind: 'productAction';
  actionId: ProductActionId;
};

export type InteractiveCapabilityOptionHandler =
  | { kind: 'config'; path: string }
  | { kind: 'mergeConfig'; path: string; fields: string[] }
  | { kind: 'appearanceSelection' }
  | { kind: 'language' };

export interface InteractiveCapabilityOperation {
  id: string;
  titleZh: string;
  titleEn: string;
  descriptionZh: string;
  descriptionEn: string;
  risk: InteractiveCapabilityRisk;
  inputSchema: Record<string, unknown>;
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
  counts: {
    features: number;
    settings: number;
    userFacing: number;
    documentedItems: number;
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
