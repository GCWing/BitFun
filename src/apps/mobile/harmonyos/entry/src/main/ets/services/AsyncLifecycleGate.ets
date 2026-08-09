/**
 * Invalidates asynchronous work when the owning UI lifecycle changes.
 * A token is intentionally cheap to copy and can be checked after every await.
 */
export class AsyncLifecycleGate {
  private generation: number = 0;

  begin(): number {
    this.generation += 1;
    return this.generation;
  }

  invalidate(): void {
    this.generation += 1;
  }

  isCurrent(token: number): boolean {
    return token === this.generation;
  }
}
