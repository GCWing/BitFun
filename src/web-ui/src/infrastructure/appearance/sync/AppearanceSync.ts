import type { AppearanceSelectionId } from '../types';

export type AppearanceCommittedEvent =
  | {
    kind: 'selection-changed';
    eventId: string;
    sourceId: string;
    selectionId: AppearanceSelectionId;
  }
  | {
    kind: 'package-upserted';
    eventId: string;
    sourceId: string;
    packageId: string;
    version: string;
  }
  | {
    kind: 'package-deleted';
    eventId: string;
    sourceId: string;
    packageId: string;
  };

export interface AppearanceSyncPort {
  publish(event: AppearanceCommittedEvent): void | Promise<void>;
  subscribe(listener: (event: AppearanceCommittedEvent) => void): () => void;
  dispose?(): void;
}

export const noopAppearanceSyncPort: AppearanceSyncPort = {
  publish: () => undefined,
  subscribe: () => () => undefined,
};

function isCommittedEvent(value: unknown): value is AppearanceCommittedEvent {
  if (!value || typeof value !== 'object' || Array.isArray(value)) return false;
  const event = value as Record<string, unknown>;
  if (typeof event.eventId !== 'string' || typeof event.sourceId !== 'string') return false;
  if (event.kind === 'selection-changed') return typeof event.selectionId === 'string';
  if (event.kind === 'package-upserted') {
    return typeof event.packageId === 'string' && typeof event.version === 'string';
  }
  return event.kind === 'package-deleted' && typeof event.packageId === 'string';
}

class BroadcastChannelAppearanceSyncPort implements AppearanceSyncPort {
  private readonly listeners = new Set<(event: AppearanceCommittedEvent) => void>();

  constructor(private readonly channel: BroadcastChannel) {
    channel.addEventListener('message', this.handleMessage);
  }

  publish(event: AppearanceCommittedEvent): void {
    this.channel.postMessage(event);
  }

  subscribe(listener: (event: AppearanceCommittedEvent) => void): () => void {
    this.listeners.add(listener);
    return () => this.listeners.delete(listener);
  }

  dispose(): void {
    this.channel.removeEventListener('message', this.handleMessage);
    this.channel.close();
    this.listeners.clear();
  }

  private readonly handleMessage = (message: MessageEvent<unknown>): void => {
    const event = message.data;
    if (!isCommittedEvent(event)) return;
    this.listeners.forEach(listener => listener(event));
  };
}

export function createAppearanceSyncPort(): AppearanceSyncPort {
  if (typeof BroadcastChannel === 'undefined') return noopAppearanceSyncPort;
  try {
    return new BroadcastChannelAppearanceSyncPort(new BroadcastChannel('openbitfun-appearance'));
  } catch {
    return noopAppearanceSyncPort;
  }
}
