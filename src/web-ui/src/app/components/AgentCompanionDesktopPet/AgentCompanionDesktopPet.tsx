import React, { useEffect, useLayoutEffect, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { listen } from '@tauri-apps/api/event';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { aiExperienceConfigService, type AgentCompanionPetSelection, type AIExperienceSettings } from '@/infrastructure/config/services/AIExperienceConfigService';
import { ChatInputPixelPet, type ChatInputPixelPetMood } from '@/flow_chat/components/ChatInputPixelPet';
import type { ChatInputPetMood } from '@/flow_chat/utils/chatInputPetMood';
import type { AgentCompanionActivityPayload, AgentCompanionTaskStatus } from '@/flow_chat/utils/agentCompanionActivity';
import { createLogger } from '@/shared/utils/logger';
import './AgentCompanionDesktopPet.scss';

const log = createLogger('AgentCompanionDesktopPet');
const PET_SIZE = 96;
const WINDOW_WIDTH_WITH_BUBBLES = 360;
const WINDOW_MAX_HEIGHT = 240;
const WINDOW_HORIZONTAL_GAP = 8;
const MAX_VISIBLE_BUBBLES = 3;
const BUBBLE_GAP = 6;

export const AgentCompanionDesktopPet: React.FC = () => {
  const { t } = useTranslation('flow-chat');
  const [pet, setPet] = useState<AgentCompanionPetSelection | null>(
    () => aiExperienceConfigService.getSettings().agent_companion_pet ?? null,
  );
  const [mood, setMood] = useState<ChatInputPetMood>('rest');
  const [tasks, setTasks] = useState<AgentCompanionTaskStatus[]>([]);
  const [isHoveringPet, setIsHoveringPet] = useState(false);
  const [isDraggingPet, setIsDraggingPet] = useState(false);
  const bubblesRef = useRef<HTMLDivElement>(null);
  const displayTasks = [...tasks].reverse();

  useEffect(() => {
    document.documentElement.classList.add('bitfun-agent-companion-window-root');
    document.body.classList.add('bitfun-agent-companion-window-body');

    const applySettings = (settings: AIExperienceSettings) => {
      setPet(settings.agent_companion_pet ?? null);
      if (!settings.enable_agent_companion || settings.agent_companion_display_mode !== 'desktop') {
        void getCurrentWindow().close();
      }
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
      setMood(event.payload.mood);
      setTasks(event.payload.tasks);
    }).then(unlisten => {
      removeActivityListener = unlisten;
    }).catch(error => {
      log.warn('Failed to listen for Agent companion activity updates', error);
    });

    const removeListener = aiExperienceConfigService.addChangeListener(settings => {
      applySettings(settings);
    });

    return () => {
      removeListener();
      removeTauriListener?.();
      removeActivityListener?.();
      document.documentElement.classList.remove('bitfun-agent-companion-window-root');
      document.body.classList.remove('bitfun-agent-companion-window-body');
    };
  }, []);

  useLayoutEffect(() => {
    const bubbleCount = tasks.length;
    const nextWidth = bubbleCount > 0 ? WINDOW_WIDTH_WITH_BUBBLES : PET_SIZE;
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
      ? Math.max(PET_SIZE, Math.min(WINDOW_MAX_HEIGHT, targetBubbleHeight))
      : PET_SIZE;

    void import('@tauri-apps/api/core')
      .then(({ invoke }) => invoke('resize_agent_companion_desktop_pet', {
        width: nextWidth,
        height: nextHeight,
      }))
      .catch(error => {
        log.warn('Failed to resize Agent companion window', error);
      });
  }, [tasks]);

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

  return (
    <main
      className="bitfun-agent-companion-window"
      onDoubleClick={() => void getCurrentWindow().close()}
      title="Double-click to close"
    >
      {tasks.length > 0 && (
        <div
          ref={bubblesRef}
          className="bitfun-agent-companion-window__bubbles"
          aria-live="polite"
          onDoubleClick={event => event.stopPropagation()}
          style={{
            '--bitfun-agent-companion-pet-size': `${PET_SIZE}px`,
            '--bitfun-agent-companion-gap': `${WINDOW_HORIZONTAL_GAP}px`,
          } as React.CSSProperties}
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
          className="bitfun-agent-companion-window__pet"
        />
      </div>
    </main>
  );
};

export default AgentCompanionDesktopPet;
