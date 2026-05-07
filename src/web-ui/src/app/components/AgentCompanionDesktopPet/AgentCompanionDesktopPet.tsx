import React, { useEffect, useState } from 'react';
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

export const AgentCompanionDesktopPet: React.FC = () => {
  const { t } = useTranslation('flow-chat');
  const [pet, setPet] = useState<AgentCompanionPetSelection | null>(
    () => aiExperienceConfigService.getSettings().agent_companion_pet ?? null,
  );
  const [mood, setMood] = useState<ChatInputPetMood>('rest');
  const [tasks, setTasks] = useState<AgentCompanionTaskStatus[]>([]);
  const [isHoveringPet, setIsHoveringPet] = useState(false);
  const [isDraggingPet, setIsDraggingPet] = useState(false);

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

  return (
    <main
      className="bitfun-agent-companion-window"
      onDoubleClick={() => void getCurrentWindow().close()}
      title="Double-click to close"
    >
      {tasks.length > 0 && (
        <div className="bitfun-agent-companion-window__bubbles" aria-live="polite">
          {tasks.map(task => (
            <div
              key={`${task.sessionId}-${task.state}`}
              className={`bitfun-agent-companion-window__bubble bitfun-agent-companion-window__bubble--${task.state}`}
            >
              <span className="bitfun-agent-companion-window__bubble-title">
                {task.title}
              </span>
              <span className="bitfun-agent-companion-window__bubble-status">
                {t(task.labelKey, { defaultValue: task.defaultLabel })}
              </span>
            </div>
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
