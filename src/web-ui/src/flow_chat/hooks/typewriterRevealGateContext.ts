import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useState,
} from 'react';

export interface TypewriterRevealGateValue {
  report: (key: string, revealing: boolean) => void;
  isAnyRevealing: boolean;
}

export const TypewriterRevealGateContext =
  createContext<TypewriterRevealGateValue | null>(null);

export function useCreateTypewriterRevealGate(): TypewriterRevealGateValue {
  const [revealingKeys, setRevealingKeys] = useState<Set<string>>(() => new Set());

  const report = useCallback((key: string, revealing: boolean) => {
    setRevealingKeys((previous) => {
      const hasKey = previous.has(key);
      if (revealing === hasKey) {
        return previous;
      }
      const next = new Set(previous);
      if (revealing) {
        next.add(key);
      } else {
        next.delete(key);
      }
      return next;
    });
  }, []);

  return useMemo<TypewriterRevealGateValue>(() => ({
    report,
    isAnyRevealing: revealingKeys.size > 0,
  }), [report, revealingKeys]);
}

export function useTypewriterRevealGate(): TypewriterRevealGateValue | null {
  return useContext(TypewriterRevealGateContext);
}

/** Report a typewriter reveal key for the lifetime of `isRevealing`. */
export function useReportTypewriterReveal(key: string, isRevealing: boolean): void {
  const gate = useTypewriterRevealGate();
  const report = gate?.report;

  useEffect(() => {
    if (!report) {
      return;
    }
    report(key, isRevealing);
    return () => {
      report(key, false);
    };
  }, [report, isRevealing, key]);
}
