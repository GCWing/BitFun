export function createBlockId(): string {
  if (typeof crypto !== 'undefined' && typeof crypto.randomUUID === 'function') {
    return crypto.randomUUID();
  }

  return `block-${Date.now()}-${Math.random().toString(36).slice(2, 10)}`;
}

