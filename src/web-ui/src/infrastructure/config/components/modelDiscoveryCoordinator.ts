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
