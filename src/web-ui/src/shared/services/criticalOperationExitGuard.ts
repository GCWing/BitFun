export type CriticalOperationExitGuard = () => Promise<boolean>;

const guards = new Set<CriticalOperationExitGuard>();

export function registerCriticalOperationExitGuard(
  guard: CriticalOperationExitGuard,
): () => void {
  guards.add(guard);
  return () => guards.delete(guard);
}

export async function confirmCriticalOperationExit(): Promise<boolean> {
  for (const guard of Array.from(guards)) {
    try {
      if (!(await guard())) return false;
    } catch {
      return false;
    }
  }
  return true;
}
