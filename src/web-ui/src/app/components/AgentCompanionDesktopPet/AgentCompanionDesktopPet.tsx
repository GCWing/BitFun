import React, { useCallback, useEffect, useLayoutEffect, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { listen } from '@tauri-apps/api/event';
import { cursorPosition, getCurrentWindow } from '@tauri-apps/api/window';
import { aiExperienceConfigService, type AgentCompanionPetSelection, type AIExperienceSettings } from '@/infrastructure/config/services/AIExperienceConfigService';
import { ChatInputPixelPet, type ChatInputPixelPetMood } from '@/flow_chat/components/ChatInputPixelPet';
import type { ChatInputPetMood } from '@/flow_chat/utils/chatInputPetMood';
import type { AgentCompanionActivityPayload, AgentCompanionTaskStatus } from '@/flow_chat/utils/agentCompanionActivity';
import { createLogger } from '@/shared/utils/logger';
import './AgentCompanionDesktopPet.scss';

const log = createLogger('AgentCompanionDesktopPet');
const DEFAULT_PET_SIZE = 96;
const DEFAULT_PETDEX_DISPLAY_SIZE = { width: 96, height: 104 };
const PETDEX_DESKTOP_SCALE = 0.5;
const WINDOW_MAX_WIDTH = 360;
const WINDOW_MAX_HEIGHT = 240;
const WINDOW_HORIZONTAL_GAP = 8;
const MAX_VISIBLE_BUBBLES = 2;
const BUBBLE_GAP = 6;
const BUBBLE_MIN_WIDTH = 132;
const BUBBLE_MAX_WIDTH = 252;
const WINDOW_EDGE_BUFFER = 4;
const POINTER_HOVER_POLL_INTERVAL_MS = 120;

export const AgentCompanionDesktopPet: React.FC = () => {
  const { t } = useTranslation('flow-chat');
  const [pet, setPet] = useState<AgentCompanionPetSelection | null>(
    () => aiExperienceConfigService.getSettings().agent_companion_pet ?? null,
  );
  const [mood, setMood] = useState<ChatInputPetMood>('rest');
  const [tasks, setTasks] = useState<AgentCompanionTaskStatus[]>([]);
  const [isHoveringPet, setIsHoveringPet] = useState(false);
  const [isDraggingPet, setIsDraggingPet] = useState(false);
  const [petFrameSize, setPetFrameSize] = useState<{ width: number; height: number } | null>(null);
  const dockRef = useRef<HTMLDivElement>(null);
  const bubblesRef = useRef<HTMLDivElement>(null);
  const lastActivitySequenceRef = useRef(0);
  const lastActivityEmittedAtRef = useRef(0);
  const displayTasks = [...tasks].reverse();
  const activePetSize = pet && petFrameSize
    ? petFrameSize
    : pet
      ? DEFAULT_PETDEX_DISPLAY_SIZE
      : { width: DEFAULT_PET_SIZE, height: DEFAULT_PET_SIZE };

  useEffect(() => {
    document.documentElement.classList.add('bitfun-agent-companion-window-root');
    document.body.classList.add('bitfun-agent-companion-window-body');

    const applySettings = (settings: AIExperienceSettings) => {
      setPet(settings.agent_companion_pet ?? null);
      setPetFrameSize(null);
    };

    void aiExperienceConfigService.getSettingsAsync().then(settings => {
      applySettings(settings);
    });

    let removeTauriListener: (() => void) | null = null;
    void listen<AIExperienceSettings>('agent-companion://settings-updated', event => {
      applySettings(event.payload);
    }).then(unlisten => {
      removeTauriListener = unlisten;
    }).catch(error => {
      log.warn('Failed to listen for Agent companion settings updates', error);
    });

    let removeActivityListener: (() => void) | null = null;
    void listen<AgentCompanionActivityPayload>('agent-companion://activity-updated', event => {
      const emittedAt = event.payload.emittedAt ?? 0;
      const sequence = event.payload.sequence ?? 0;
      if (
        emittedAt < lastActivityEmittedAtRef.current
        || (emittedAt === lastActivityEmittedAtRef.current && sequence <= lastActivitySequenceRef.current)
      ) {
        return;
      }
      lastActivityEmittedAtRef.current = emittedAt;
      lastActivitySequenceRef.current = sequence;
      setMood(event.payload.mood);
      setTasks(event.payload.tasks);
    }).then(unlisten => {
      removeActivityListener = unlisten;
    }).catch(error => {
      log.warn('Failed to listen for Agent companion activity updates', error);
    });

    return () => {
      removeTauriListener?.();
      removeActivityListener?.();
      document.documentElement.classList.remove('bitfun-agent-companion-window-root');
      document.body.classList.remove('bitfun-agent-companion-window-body');
    };
  }, []);

  useLayoutEffect(() => {
    const bubbleCount = tasks.length;
    const bubbleElements = Array.from(bubblesRef.current?.children ?? [])
      .slice(0, MAX_VISIBLE_BUBBLES);
    const visibleBubbleHeight = bubbleElements.reduce(
      (sum, child) => sum + child.getBoundingClientRect().height,
      0,
    ) + Math.max(0, bubbleElements.length - 1) * BUBBLE_GAP;
    const measuredBubbleHeight = bubblesRef.current?.scrollHeight ?? 0;
    const targetBubbleHeight = bubbleCount > MAX_VISIBLE_BUBBLES
      ? visibleBubbleHeight
      : measuredBubbleHeight;
    const nextHeight = bubbleCount > 0
      ? Math.max(activePetSize.height, Math.min(WINDOW_MAX_HEIGHT, targetBubbleHeight))
      : activePetSize.height;
    const measuredBubbleWidth = bubbleCount > 0
      ? Math.min(
        BUBBLE_MAX_WIDTH,
        Math.max(
          BUBBLE_MIN_WIDTH,
          bubblesRef.current?.scrollWidth ?? 0,
          bubblesRef.current?.getBoundingClientRect().width ?? 0,
          ...Array.from(bubblesRef.current?.children ?? []).map(child => {
            const element = child as HTMLElement;
            return Math.max(element.scrollWidth, element.getBoundingClientRect().width);
          }),
        ),
      )
      : 0;
    const measuredDockWidth = bubbleCount > 0
      ? measuredBubbleWidth + WINDOW_HORIZONTAL_GAP + activePetSize.width + WINDOW_EDGE_BUFFER
      : Math.max(
        activePetSize.width,
        dockRef.current?.scrollWidth ?? 0,
        dockRef.current?.getBoundingClientRect().width ?? 0,
      );
    const nextWidth = Math.max(
      activePetSize.width,
      Math.min(WINDOW_MAX_WIDTH, Math.ceil(measuredDockWidth)),
    );

    void import('@tauri-apps/api/core')
      .then(({ invoke }) => invoke('resize_agent_companion_desktop_pet', {
        width: nextWidth,
        height: nextHeight,
      }))
      .catch(error => {
        log.warn('Failed to resize Agent companion window', error);
      });
  }, [activePetSize.height, activePetSize.width, tasks]);

  useEffect(() => {
    const tauriWindow = getCurrentWindow();
    let disposed = false;
    let windowPosition: { x: number; y: number } | null = null;
    let scaleFactor = 1;
    let removeWindowMovedListener: (() => void) | null = null;
    let removeScaleChangedListener: (() => void) | null = null;

    void tauriWindow.outerPosition()
      .then(position => {
        windowPosition = position;
      })
      .catch(error => {
        log.warn('Failed to read Agent companion window position', error);
      });

    void tauriWindow.scaleFactor()
      .then(nextScaleFactor => {
        scaleFactor = nextScaleFactor;
      })
      .catch(error => {
        log.warn('Failed to read Agent companion window scale factor', error);
      });

    void tauriWindow.onMoved(event => {
      windowPosition = event.payload;
    }).then(unlisten => {
      if (disposed) {
        unlisten();
      } else {
        removeWindowMovedListener = unlisten;
      }
    }).catch(error => {
      log.warn('Failed to listen for Agent companion window moves', error);
    });

    void tauriWindow.onScaleChanged(event => {
      scaleFactor = event.payload.scaleFactor;
    }).then(unlisten => {
      if (disposed) {
        unlisten();
      } else {
        removeScaleChangedListener = unlisten;
      }
    }).catch(error => {
      log.warn('Failed to listen for Agent companion scale changes', error);
    });

    const pollPointerHover = async () => {
      try {
        if (!windowPosition) {
          windowPosition = await tauriWindow.outerPosition();
        }

        const pointer = await cursorPosition();
        if (disposed) {
          return;
        }

        const hitbox = dockRef.current?.querySelector<HTMLElement>('.bitfun-agent-companion-window__pet-hitbox');
        if (!hitbox) {
          setIsHoveringPet(false);
          return;
        }

        const hitboxRect = hitbox.getBoundingClientRect();
        const pointerX = (pointer.x - windowPosition.x) / scaleFactor;
        const pointerY = (pointer.y - windowPosition.y) / scaleFactor;
        const isPointerInsideHitbox = pointerX >= hitboxRect.left
          && pointerX <= hitboxRect.right
          && pointerY >= hitboxRect.top
          && pointerY <= hitboxRect.bottom;

        setIsHoveringPet(isPointerInsideHitbox);
      } catch (error) {
        log.warn('Failed to poll Agent companion pointer hover state', error);
      }
    };

    const intervalId = window.setInterval(() => {
      void pollPointerHover();
    }, POINTER_HOVER_POLL_INTERVAL_MS);
    void pollPointerHover();

    return () => {
      disposed = true;
      window.clearInterval(intervalId);
      removeWindowMovedListener?.();
      removeScaleChangedListener?.();
    };
  }, []);

  const startDrag = (event: React.PointerEvent<HTMLDivElement>) => {
    if (event.button !== 0) {
      return;
    }

    event.preventDefault();
    setIsDraggingPet(true);
    void getCurrentWindow().startDragging()
      .catch(error => {
        log.warn('Failed to start Agent companion window drag', error);
      })
      .finally(() => {
        setIsDraggingPet(false);
      });
  };

  const displayMood: ChatInputPixelPetMood = isDraggingPet
    ? 'dragging'
    : isHoveringPet
      ? 'hover'
      : mood;

  const openTaskSession = async (task: AgentCompanionTaskStatus) => {
    try {
      const [{ invoke }, { emit }] = await Promise.all([
        import('@tauri-apps/api/core'),
        import('@tauri-apps/api/event'),
      ]);
      await emit('agent-companion://open-session', { sessionId: task.sessionId });
      await invoke('show_main_window');
    } catch (error) {
      log.warn('Failed to open Agent companion task session', {
        sessionId: task.sessionId,
        error,
      });
    }
  };

  const handlePetFrameSizeChange = useCallback((size: { width: number; height: number } | null) => {
    setPetFrameSize(size);
  }, []);

  const dockVars = {
    '--bitfun-agent-companion-pet-width': `${activePetSize.width}px`,
    '--bitfun-agent-companion-pet-height': `${activePetSize.height}px`,
    '--bitfun-agent-companion-gap': `${WINDOW_HORIZONTAL_GAP}px`,
  } as React.CSSProperties;

  return (
    <main
      className="bitfun-agent-companion-window"
    >
      <div
        ref={dockRef}
        className="bitfun-agent-companion-window__dock"
        style={dockVars}
      >
        {tasks.length > 0 && (
          <div
            ref={bubblesRef}
            className="bitfun-agent-companion-window__bubbles"
            aria-live="polite"
            onDoubleClick={event => event.stopPropagation()}
          >
            {displayTasks.map(task => (
              <button
                type="button"
                key={task.sessionId}
                className={`bitfun-agent-companion-window__bubble bitfun-agent-companion-window__bubble--${task.state}`}
                onClick={() => void openTaskSession(task)}
              >
                <span className="bitfun-agent-companion-window__bubble-title">
                  {task.title}
                </span>
                <span className="bitfun-agent-companion-window__bubble-status">
                  {t(task.labelKey, { defaultValue: task.defaultLabel })}
                </span>
              </button>
            ))}
          </div>
        )}
        <div
          className="bitfun-agent-companion-window__pet-hitbox"
          onPointerEnter={() => setIsHoveringPet(true)}
          onPointerLeave={() => setIsHoveringPet(false)}
          onPointerDown={startDrag}
        >
          <ChatInputPixelPet
            mood={displayMood}
            pet={pet}
            nativePetdexSize
            petdexScale={PETDEX_DESKTOP_SCALE}
            onPetFrameSizeChange={handlePetFrameSizeChange}
            className="bitfun-agent-companion-window__pet"
          />
        </div>
      </div>
    </main>
  );
};

export default AgentCompanionDesktopPet;
