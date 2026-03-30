import { createContext, useContext } from 'react';

export type ToolCardHeaderAffordanceKind = 'expand' | 'open-panel-right';

export interface ToolCardHeaderLayoutContextValue {
  headerExpandAffordance: boolean;
  headerAffordanceKind: ToolCardHeaderAffordanceKind;
  isExpanded: boolean;
}

export const ToolCardHeaderLayoutContext = createContext<ToolCardHeaderLayoutContextValue>({
  headerExpandAffordance: false,
  headerAffordanceKind: 'expand',
  isExpanded: false,
});

export function useToolCardHeaderLayout(): ToolCardHeaderLayoutContextValue {
  return useContext(ToolCardHeaderLayoutContext);
}
