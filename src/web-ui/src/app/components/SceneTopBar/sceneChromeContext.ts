import { createContext, useContext, type ReactNode } from 'react';
import type { SceneTabId } from '../SceneBar/types';

export interface SceneChromeContextValue {
  activeSceneId: SceneTabId | null;
  setContribution: (
    sceneId: SceneTabId,
    owner: symbol,
    content: ReactNode,
  ) => void;
  removeContribution: (sceneId: SceneTabId, owner: symbol) => void;
}

export const SceneChromeContext = createContext<SceneChromeContextValue | null>(null);
export function useSceneChromeContext(): SceneChromeContextValue | null {
  return useContext(SceneChromeContext);
}
