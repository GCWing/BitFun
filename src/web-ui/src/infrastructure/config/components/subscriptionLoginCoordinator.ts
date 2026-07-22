import type { SubscriptionProvider } from '../types';

export interface SubscriptionLoginOperation {
  id: number;
  provider: SubscriptionProvider;
  cancelled: boolean;
  startSettled: boolean;
}

export interface SubscriptionStartSettlement {
  shouldContinue: boolean;
  cleanupError?: unknown;
}

/**
 * Owns the single active subscription authorization operation.
 *
 * Keeping operation identity outside React state prevents a stale async
 * `finally` block from clearing a newer login session.
 */
export class SubscriptionLoginCoordinator {
  private nextId = 0;
  private active: SubscriptionLoginOperation | null = null;

  begin(provider: SubscriptionProvider): SubscriptionLoginOperation | null {
    if (this.active) return null;
    const operation: SubscriptionLoginOperation = {
      id: ++this.nextId,
      provider,
      cancelled: false,
      startSettled: false,
    };
    this.active = operation;
    return operation;
  }

  current(): SubscriptionLoginOperation | null {
    return this.active;
  }

  isCurrent(operation: SubscriptionLoginOperation): boolean {
    return this.active === operation && !operation.cancelled;
  }

  owns(operation: SubscriptionLoginOperation): boolean {
    return this.active === operation;
  }

  markStartSettled(operation: SubscriptionLoginOperation): boolean {
    if (this.active !== operation) return false;
    operation.startSettled = true;
    return !operation.cancelled;
  }

  requestCancel(provider: SubscriptionProvider): SubscriptionLoginOperation | null {
    if (!this.active || this.active.provider !== provider) return null;
    const operation = this.active;
    operation.cancelled = true;
    return operation;
  }

  complete(operation: SubscriptionLoginOperation): boolean {
    if (this.active !== operation) return false;
    this.active = null;
    return true;
  }
}

/**
 * Resolves the start/cancel race. A cancellation requested while the backend
 * start command is still running keeps the coordinator slot reserved; once
 * start returns, this helper cancels the now-created backend session before
 * the operation may be completed.
 */
export async function settleSubscriptionLoginStart(
  coordinator: SubscriptionLoginCoordinator,
  operation: SubscriptionLoginOperation,
  cancelBackend: () => Promise<void>,
): Promise<SubscriptionStartSettlement> {
  if (!coordinator.owns(operation)) {
    return { shouldContinue: false };
  }
  if (coordinator.markStartSettled(operation)) {
    return { shouldContinue: true };
  }
  try {
    await cancelBackend();
    return { shouldContinue: false };
  } catch (cleanupError) {
    return { shouldContinue: false, cleanupError };
  }
}
