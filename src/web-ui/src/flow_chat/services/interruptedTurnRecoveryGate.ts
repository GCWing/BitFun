export interface InterruptedTurnRecoveryOperation {
  sessionId: string;
  turnId: string;
  executionGeneration: number;
}

type Listener = () => void;

function sameOperation(
  left: InterruptedTurnRecoveryOperation,
  right: InterruptedTurnRecoveryOperation,
): boolean {
  return left.sessionId === right.sessionId
    && left.turnId === right.turnId
    && left.executionGeneration === right.executionGeneration;
}

class InterruptedTurnRecoveryGate {
  private readonly operations = new Map<string, InterruptedTurnRecoveryOperation>();
  private readonly listeners = new Set<Listener>();
  private version = 0;

  public readonly subscribe = (listener: Listener): (() => void) => {
    this.listeners.add(listener);
    return () => this.listeners.delete(listener);
  };

  public readonly getSnapshot = (): number => this.version;

  public tryBegin(operation: InterruptedTurnRecoveryOperation): boolean {
    if (this.operations.has(operation.sessionId)) return false;
    this.operations.set(operation.sessionId, operation);
    this.emit();
    return true;
  }

  public isSessionInFlight(sessionId: string | null | undefined): boolean {
    return Boolean(sessionId && this.operations.has(sessionId));
  }

  public clearExact(operation: InterruptedTurnRecoveryOperation): void {
    const current = this.operations.get(operation.sessionId);
    if (!current || !sameOperation(current, operation)) return;
    this.operations.delete(operation.sessionId);
    this.emit();
  }

  public clearRecovered(operation: InterruptedTurnRecoveryOperation): void {
    this.clearNextGeneration(operation);
  }

  public clearInterrupted(operation: InterruptedTurnRecoveryOperation): void {
    this.clearNextGeneration(operation);
  }

  public clearTerminal(sessionId: string, turnId: string): void {
    const current = this.operations.get(sessionId);
    if (!current || current.turnId !== turnId) return;
    this.operations.delete(sessionId);
    this.emit();
  }

  public resetForTests(): void {
    if (this.operations.size === 0) return;
    this.operations.clear();
    this.emit();
  }

  private clearNextGeneration(operation: InterruptedTurnRecoveryOperation): void {
    const current = this.operations.get(operation.sessionId);
    if (
      !current
      || current.turnId !== operation.turnId
      || operation.executionGeneration <= current.executionGeneration
    ) {
      return;
    }
    this.operations.delete(operation.sessionId);
    this.emit();
  }

  private emit(): void {
    this.version += 1;
    for (const listener of this.listeners) listener();
  }
}

export const interruptedTurnRecoveryGate = new InterruptedTurnRecoveryGate();
