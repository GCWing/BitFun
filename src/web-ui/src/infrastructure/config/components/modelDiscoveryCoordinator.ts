import type { SubscriptionApiOffering } from '@/infrastructure/api/service-api/AIApi';
import type { OpenCodePlan } from '../types';

interface DiscoveryOperation {
  signature: string;
}

/** Discards late results after a provider/format change or editor reset. */
export class ModelDiscoveryCoordinator {
  private active: DiscoveryOperation | null = null;
  private completed: string | null = null;

  begin(signature: string, force = false): DiscoveryOperation | null {
    if (this.active?.signature === signature || (!force && this.completed === signature)) return null;
    const operation = { signature };
    this.active = operation;
    return operation;
  }

  isCurrent(operation: DiscoveryOperation): boolean {
    return this.active === operation;
  }

  complete(operation: DiscoveryOperation, succeeded: boolean): boolean {
    if (!this.isCurrent(operation)) return false;
    this.completed = succeeded ? operation.signature : null;
    this.active = null;
    return true;
  }

  reset(): void {
    this.active = null;
    this.completed = null;
  }
}

/** The picker exposes models; the account catalog supplies each model's wire. */
export function openCodeOfferingModels(
  offerings: SubscriptionApiOffering[],
  plan: OpenCodePlan | undefined,
) {
  const models = offerings.filter(item => item.plan === (plan ?? 'zen')).flatMap(item => item.models);
  return models.filter((model, index) => models.findIndex(item => item.id === model.id) === index);
}

export function openCodeModelOffering(
  offerings: SubscriptionApiOffering[],
  plan: OpenCodePlan | undefined,
  model: string,
  configuredFormat = 'openai',
) {
  const matches = offerings.filter(item => item.plan === (plan ?? 'zen')
    && item.models.some(entry => entry.id === model.trim()));
  const format = plan ? (configuredFormat === 'response' ? 'responses' : configuredFormat) : 'openai';
  return matches.find(item => item.format === format) ?? matches[0];
}
