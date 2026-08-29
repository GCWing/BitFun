import React, {
  createContext,
  useCallback,
  useContext,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
  type HTMLAttributes,
  type ReactNode,
} from 'react';
import type { SceneTabId } from '../SceneBar/types';

interface SceneChromeContributionRecord {
  owner: symbol;
  content: ReactNode;
}

interface SceneChromeContextValue {
  activeSceneId: SceneTabId | null;
  setContribution: (
    sceneId: SceneTabId,
    owner: symbol,
    content: ReactNode,
  ) => void;
  removeContribution: (sceneId: SceneTabId, owner: symbol) => void;
}

const SceneChromeContext = createContext<SceneChromeContextValue | null>(null);
const SceneChromeContentContext = createContext<ReactNode>(null);

interface SceneChromeProviderProps {
  activeSceneId: SceneTabId | null;
  children: ReactNode;
}

export const SceneChromeProvider: React.FC<SceneChromeProviderProps> = ({
  activeSceneId,
  children,
}) => {
  const [contributions, setContributions] = useState(
    () => new Map<SceneTabId, SceneChromeContributionRecord>(),
  );
  const setContribution = useCallback((
    sceneId: SceneTabId,
    owner: symbol,
    content: ReactNode,
  ) => {
    setContributions(current => {
      const existing = current.get(sceneId);
      if (existing?.owner === owner && existing.content === content) {
        return current;
      }

      const next = new Map(current);
      next.set(sceneId, { owner, content });
      return next;
    });
  }, []);
  const removeContribution = useCallback((sceneId: SceneTabId, owner: symbol) => {
    setContributions(current => {
      if (current.get(sceneId)?.owner !== owner) {
        return current;
      }

      const next = new Map(current);
      next.delete(sceneId);
      return next;
    });
  }, []);
  const registrationValue = useMemo<SceneChromeContextValue>(() => ({
    activeSceneId,
    setContribution,
    removeContribution,
  }), [activeSceneId, removeContribution, setContribution]);
  const activeContent = activeSceneId
    ? contributions.get(activeSceneId)?.content ?? null
    : null;

  return (
    <SceneChromeContext.Provider value={registrationValue}>
      <SceneChromeContentContext.Provider value={activeContent}>
        {children}
      </SceneChromeContentContext.Provider>
    </SceneChromeContext.Provider>
  );
};

type SceneChromeHostProps = Omit<HTMLAttributes<HTMLDivElement>, 'children'>;

export const SceneChromeHost: React.FC<SceneChromeHostProps> = (props) => {
  const activeContent = useContext(SceneChromeContentContext);
  return <div {...props}>{activeContent}</div>;
};

interface SceneChromeContributionProps {
  sceneId: SceneTabId;
  children: ReactNode;
}

export const SceneChromeContribution: React.FC<SceneChromeContributionProps> = ({
  sceneId,
  children,
}) => {
  const sceneChrome = useContext(SceneChromeContext);
  const ownerRef = useRef(Symbol('scene-chrome-contribution'));
  const setContribution = sceneChrome?.setContribution;
  const removeContribution = sceneChrome?.removeContribution;

  useLayoutEffect(() => {
    setContribution?.(sceneId, ownerRef.current, children);
  }, [children, sceneId, setContribution]);

  useLayoutEffect(() => () => {
    removeContribution?.(sceneId, ownerRef.current);
  }, [removeContribution, sceneId]);

  return null;
};

export function useSceneChromeContext(): SceneChromeContextValue | null {
  return useContext(SceneChromeContext);
}
