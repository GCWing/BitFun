/**
 * Lets nested typewriter consumers report whether they are still revealing,
 * so parents (e.g. ModelRoundItem footer) can wait for visual completion.
 */

import React, {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useState,
  type ReactNode,
} from 'react';

interface TypewriterRevealGateValue {
  report: (key: string, revealing: boolean) => void;
  isAnyRevealing: boolean;
}

const TypewriterRevealGateContext = createContext<TypewriterRevealGateValue | null>(null);

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

export const TypewriterRevealGateProvider: React.FC<{
  value?: TypewriterRevealGateValue;
  children: ReactNode;
}> = ({ value, children }) => {
  const localValue = useCreateTypewriterRevealGate();
  return (
    <TypewriterRevealGateContext.Provider value={value ?? localValue}>
      {children}
    </TypewriterRevealGateContext.Provider>
  );
};

export function useTypewriterRevealGate(): TypewriterRevealGateValue | null {
  return useContext(TypewriterRevealGateContext);
}

/**
 * Report a typewriter reveal key for the lifetime of `isRevealing === true`.
 *
 * Depends on the stable `report` function only — the gate value object gets a
 * new identity on every reported change, so depending on it would re-run the
 * effect on each report: cleanup removes the key, the body re-adds it, and
 * the pair of state updates changes the gate identity again (infinite loop).
 */
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
