// @vitest-environment jsdom

import React, { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { I18nextProvider, initReactI18next } from 'react-i18next';
import { createInstance, type i18n as I18nInstance } from 'i18next';
import { afterEach, beforeAll, beforeEach, describe, expect, it, vi } from 'vitest';
import type { AgentCompanionActivityPayload, AgentCompanionTaskStatus } from '@/flow_chat/utils/agentCompanionActivity';
import { AgentCompanionDesktopPet } from './AgentCompanionDesktopPet';

const emitMock = vi.hoisted(() => vi.fn(() => Promise.resolve()));
const listenMock = vi.hoisted(() => vi.fn());
const invokeMock = vi.hoisted(() => vi.fn(() => Promise.resolve()));
const cursorPositionMock = vi.hoisted(() => vi.fn(() => Promise.resolve({ x: 0, y: 0 })));
/** Backs the Tauri IPC bridge, so window resize requests are observable. */
const hostInvokeMock = vi.hoisted(() => vi.fn(() => Promise.resolve()));

vi.mock('@tauri-apps/api/event', () => ({
  emit: emitMock,
  listen: listenMock,
}));

vi.mock('@tauri-apps/api/core', () => ({
  invoke: invokeMock,
}));

vi.mock('@tauri-apps/api/window', () => ({
  cursorPosition: cursorPositionMock,
  getCurrentWindow: () => ({
    hide: vi.fn(() => Promise.resolve()),
    setFocus: vi.fn(() => Promise.resolve()),
    startDragging: vi.fn(() => Promise.resolve()),
    outerPosition: vi.fn(() => Promise.resolve({ x: 0, y: 0 })),
    scaleFactor: vi.fn(() => Promise.resolve(1)),
    onMoved: vi.fn(() => Promise.resolve(() => {})),
    onScaleChanged: vi.fn(() => Promise.resolve(() => {})),
  }),
}));

vi.mock('@/infrastructure/config/services/AIExperienceConfigService', () => ({
  aiExperienceConfigService: {
    getSettings: () => ({ agent_companion_pet: null }),
    getSettingsAsync: () => Promise.resolve({
      enable_agent_companion: true,
      agent_companion_pet: null,
    }),
  },
}));

vi.mock('@/flow_chat/components/AgentCompanionPet', () => ({
  AgentCompanionPet: () => <div data-testid="pixel-pet" />,
}));

const PET_COMMAND_EVENT = 'agent-companion://pet-command';
const ACTIVITY_EVENT = 'agent-companion://activity-updated';

let i18n: I18nInstance;

function task(overrides: Partial<AgentCompanionTaskStatus> = {}): AgentCompanionTaskStatus {
  return {
    sessionId: 'session-1',
    title: 'Refactor login',
    mood: 'working',
    state: 'running',
    labelKey: 'agentCompanion.activity.working',
    defaultLabel: 'Working',
    startedAt: 1,
    updatedAt: 2,
    canReply: true,
    ...overrides,
  };
}

/**
 * React caches the last value it saw on the element instance, so assigning
 * `input.value` directly is invisible to `onChange`. Going through the prototype
 * setter bypasses that cache and makes the input event look like real typing.
 */
function typeInto(input: HTMLInputElement, value: string): void {
  const nativeValueSetter = Object.getOwnPropertyDescriptor(
    window.HTMLInputElement.prototype,
    'value',
  )?.set;
  act(() => {
    nativeValueSetter?.call(input, value);
    input.dispatchEvent(new window.Event('input', { bubbles: true }));
  });
}

beforeAll(async () => {
  // The pet resizes its own window through `invoke`; the dynamic import inside
  // the component reaches the real Tauri core module, so stub the IPC bridge.
  (window as unknown as { __TAURI_INTERNALS__: unknown }).__TAURI_INTERNALS__ = {
    invoke: hostInvokeMock,
    transformCallback: (callback: unknown) => callback,
  };

  i18n = createInstance();
  await i18n.use(initReactI18next).init({
    lng: 'en-US',
    fallbackLng: 'en-US',
    resources: {
      'en-US': {
        'flow-chat': {
          agentCompanion: {
            activity: { working: 'Working', completed: 'Completed' },
            menu: {
              switchPet: 'Switch pet',
              closePet: 'Close pet',
              closeBubble: 'Close this bubble',
            },
            composer: {
              openTitle: 'Send a message to this session',
              ariaLabel: 'Send a message to this session',
              placeholder: 'Type a message, Enter to send',
              cancel: 'Cancel',
              send: 'Send',
            },
          },
        },
      },
    },
    interpolation: { escapeValue: false },
  });
});

describe('AgentCompanionDesktopPet', () => {
  let container: HTMLDivElement;
  let root: Root;
  let pushActivity: (payload: AgentCompanionActivityPayload) => void;

  const query = <T extends Element>(selector: string): T | null =>
    container.querySelector<T>(selector);

  const dispatch = (element: Element, type: string, at?: { clientX: number; clientY: number }) => {
    act(() => {
      element.dispatchEvent(new window.MouseEvent(type, {
        bubbles: true,
        cancelable: true,
        ...at,
      }));
    });
  };

  beforeEach(async () => {
    emitMock.mockClear();
    invokeMock.mockClear();
    hostInvokeMock.mockClear();
    cursorPositionMock.mockReset();
    cursorPositionMock.mockResolvedValue({ x: 0, y: 0 });
    listenMock.mockReset();

    const activityListeners: Array<(event: { payload: AgentCompanionActivityPayload }) => void> = [];
    listenMock.mockImplementation((eventName: string, handler: (event: { payload: unknown }) => void) => {
      if (eventName === ACTIVITY_EVENT) {
        activityListeners.push(handler as (event: { payload: AgentCompanionActivityPayload }) => void);
      }
      return Promise.resolve(() => {});
    });
    pushActivity = payload => {
      act(() => {
        activityListeners.forEach(handler => handler({ payload }));
      });
    };

    container = document.createElement('div');
    document.body.appendChild(container);
    await act(async () => {
      root = createRoot(container);
      root.render(
        <I18nextProvider i18n={i18n}>
          <AgentCompanionDesktopPet />
        </I18nextProvider>
      );
    });
  });

  afterEach(() => {
    act(() => {
      root.unmount();
    });
    container.remove();
  });

  it('closes the desktop pet from the pet context menu', () => {
    const hitbox = query('.openbitfun-agent-companion-window__pet-hitbox');
    expect(hitbox).not.toBeNull();

    dispatch(hitbox!, 'contextmenu', { clientX: 300, clientY: 200 });

    const menuItem = Array.from(
      container.querySelectorAll<HTMLButtonElement>('[data-openbitfun-menu-item]'),
    ).find(item => item.textContent === 'Close pet');
    expect(menuItem?.textContent).toBe('Close pet');

    act(() => {
      menuItem!.click();
    });

    expect(emitMock).toHaveBeenCalledWith(PET_COMMAND_EVENT, { type: 'close-desktop-pet' });
    expect(query('[data-openbitfun-menu-item]')).toBeNull();
  });

  it('opens the pet settings from the pet context menu', () => {
    dispatch(query('.openbitfun-agent-companion-window__pet-hitbox')!, 'contextmenu', {
      clientX: 300,
      clientY: 200,
    });

    const menuItems = Array.from(
      container.querySelectorAll<HTMLButtonElement>('[data-openbitfun-menu-item]'),
    );
    expect(menuItems.map(item => item.textContent)).toEqual(['Switch pet', 'Close pet']);

    act(() => {
      menuItems[0]!.click();
    });

    expect(emitMock).toHaveBeenCalledWith(PET_COMMAND_EVENT, { type: 'open-pet-settings' });
    expect(query('[data-openbitfun-menu-item]')).toBeNull();
  });

  it('anchors the context menu to the cursor position', () => {
    dispatch(query('.openbitfun-agent-companion-window__pet-hitbox')!, 'contextmenu', {
      clientX: 300,
      clientY: 200,
    });

    const menu = query<HTMLDivElement>('.openbitfun-agent-companion-window__overlay--anchored');
    expect(menu).not.toBeNull();
    // The anchor is measured from the bottom-right corner, which the host keeps
    // fixed while the window grows for the menu.
    expect(menu!.style.right).toBe(`${window.innerWidth - 300}px`);
    expect(menu!.style.bottom).toBe(`${window.innerHeight - 200}px`);
    expect(menu!.style.visibility).toBe('visible');
  });

  it('does not resize the window for a context menu', async () => {
    pushActivity({ mood: 'working', tasks: [task()], sequence: 1, emittedAt: 1 });
    // Let the bubble's own resize request settle before watching for new ones.
    await act(async () => {
      await Promise.resolve();
    });
    hostInvokeMock.mockClear();

    dispatch(query('.openbitfun-agent-companion-window__bubble-shell')!, 'contextmenu');
    dispatch(query('.openbitfun-agent-companion-window__pet-hitbox')!, 'contextmenu');
    await act(async () => {
      await Promise.resolve();
    });

    // Resizing moves every bottom-right anchored element for a frame, which
    // reads as the pet and bubbles flashing.
    expect(hostInvokeMock).not.toHaveBeenCalledWith(
      'resize_agent_companion_desktop_pet',
      expect.anything(),
      expect.anything(),
    );
  });

  it('sends a message from the bubble composer', async () => {
    pushActivity({ mood: 'working', tasks: [task()], sequence: 1, emittedAt: 1 });

    const composeButton = query<HTMLButtonElement>('.openbitfun-agent-companion-window__bubble-compose');
    expect(composeButton).not.toBeNull();

    act(() => {
      composeButton!.click();
    });

    // The input bar belongs to the bubble itself; no extra bubble or panel.
    expect(container.querySelectorAll('.openbitfun-agent-companion-window__bubble')).toHaveLength(1);
    const input = query<HTMLInputElement>('.openbitfun-agent-companion-window__bubble .openbitfun-agent-companion-window__bubble-composer-input');
    expect(input).not.toBeNull();
    expect(query('.openbitfun-agent-companion-window__bubble--composing')).not.toBeNull();
    expect(query('.openbitfun-agent-companion-window__bubble-compose')).toBeNull();

    typeInto(input!, '  ship it  ');
    act(() => {
      query<HTMLButtonElement>('.openbitfun-agent-companion-window__bubble-composer-send')!.click();
    });

    expect(emitMock).toHaveBeenCalledWith(PET_COMMAND_EVENT, {
      type: 'send-message',
      sessionId: 'session-1',
      message: 'ship it',
    });

    // The bar closes once the command has been handed to the main window.
    await act(async () => {
      await Promise.resolve();
    });
    expect(query('.openbitfun-agent-companion-window__bubble-composer-input')).toBeNull();
    expect(query('.openbitfun-agent-companion-window__bubble-compose')).not.toBeNull();
  });

  it('reveals the bubble entry from cursor polling before the pet window is clicked', async () => {
    pushActivity({ mood: 'working', tasks: [task()], sequence: 1, emittedAt: 1 });

    const bubbleShell = query<HTMLElement>('.openbitfun-agent-companion-window__bubble-shell');
    expect(bubbleShell).not.toBeNull();
    vi.spyOn(bubbleShell!, 'getBoundingClientRect').mockReturnValue({
      x: 10,
      y: 10,
      left: 10,
      top: 10,
      right: 110,
      bottom: 80,
      width: 100,
      height: 70,
      toJSON: () => ({}),
    });
    cursorPositionMock.mockResolvedValue({ x: 40, y: 40 });

    // No mouseover is dispatched: the transparent, inactive desktop window
    // must discover this hover through the same global cursor poll as the pet.
    await act(async () => {
      await new Promise(resolve => window.setTimeout(resolve, 150));
    });

    expect(bubbleShell!.classList).toContain('openbitfun-agent-companion-window__bubble-shell--hovered');

    cursorPositionMock.mockResolvedValue({ x: 200, y: 200 });
    await act(async () => {
      await new Promise(resolve => window.setTimeout(resolve, 150));
    });

    expect(bubbleShell!.classList).not.toContain('openbitfun-agent-companion-window__bubble-shell--hovered');
  });

  it('sends the composer message on Enter', () => {
    pushActivity({ mood: 'working', tasks: [task()], sequence: 1, emittedAt: 1 });

    act(() => {
      query<HTMLButtonElement>('.openbitfun-agent-companion-window__bubble-compose')!.click();
    });
    const input = query<HTMLInputElement>('.openbitfun-agent-companion-window__bubble-composer-input')!;
    typeInto(input, 'run the tests');

    act(() => {
      input.dispatchEvent(new window.KeyboardEvent('keydown', {
        key: 'Enter',
        bubbles: true,
        cancelable: true,
      }));
    });

    expect(emitMock).toHaveBeenCalledWith(PET_COMMAND_EVENT, {
      type: 'send-message',
      sessionId: 'session-1',
      message: 'run the tests',
    });
  });

  it('cancels the bubble composer without sending its draft', () => {
    pushActivity({ mood: 'working', tasks: [task()], sequence: 1, emittedAt: 1 });

    act(() => {
      query<HTMLButtonElement>('.openbitfun-agent-companion-window__bubble-compose')!.click();
    });
    const input = query<HTMLInputElement>('.openbitfun-agent-companion-window__bubble-composer-input')!;
    typeInto(input, 'keep this draft local');

    act(() => {
      query<HTMLButtonElement>('.openbitfun-agent-companion-window__bubble-composer-cancel')!.click();
    });

    expect(query('.openbitfun-agent-companion-window__bubble-composer-input')).toBeNull();
    expect(query('.openbitfun-agent-companion-window__bubble-compose')).not.toBeNull();
    expect(emitMock).not.toHaveBeenCalledWith(
      PET_COMMAND_EVENT,
      expect.objectContaining({ type: 'send-message' }),
    );
  });

  it('hides the composer entry for sessions that cannot be replied to directly', () => {
    pushActivity({
      mood: 'working',
      tasks: [task({ canReply: false })],
      sequence: 1,
      emittedAt: 1,
    });

    expect(query('.openbitfun-agent-companion-window__bubble')).not.toBeNull();
    expect(query('.openbitfun-agent-companion-window__bubble-compose')).toBeNull();
  });

  it('closes a finished bubble and acknowledges it in the main window', () => {
    pushActivity({
      mood: 'rest',
      tasks: [task({ state: 'completed', labelKey: 'agentCompanion.activity.completed', defaultLabel: 'Completed' })],
      sequence: 1,
      emittedAt: 1,
    });

    dispatch(query('.openbitfun-agent-companion-window__bubble-shell')!, 'contextmenu');

    const menuItem = query<HTMLButtonElement>('[data-openbitfun-menu-item]');
    expect(menuItem?.textContent).toBe('Close this bubble');
    // The bubble menu shows the action only, not the session title.
    expect(query('.openbitfun-agent-companion-window__overlay--anchored .openbitfun-agent-companion-window__overlay-title')).toBeNull();

    act(() => {
      menuItem!.click();
    });

    expect(query('.openbitfun-agent-companion-window__bubble')).toBeNull();
    expect(emitMock).toHaveBeenCalledWith(PET_COMMAND_EVENT, {
      type: 'dismiss-task',
      sessionId: 'session-1',
    });
  });

  it('keeps a silenced running bubble hidden but restores it once the task needs attention', () => {
    const runningTask = task();
    pushActivity({ mood: 'working', tasks: [runningTask], sequence: 1, emittedAt: 1 });

    dispatch(query('.openbitfun-agent-companion-window__bubble-shell')!, 'contextmenu');
    act(() => {
      query<HTMLButtonElement>('[data-openbitfun-menu-item]')!.click();
    });

    expect(query('.openbitfun-agent-companion-window__bubble')).toBeNull();
    // A running bubble is not an unread notice, so nothing is acknowledged.
    expect(emitMock).not.toHaveBeenCalledWith(PET_COMMAND_EVENT, expect.objectContaining({
      type: 'dismiss-task',
    }));

    // Still running: stays silenced.
    pushActivity({
      mood: 'working',
      tasks: [task({ updatedAt: 3 })],
      sequence: 2,
      emittedAt: 2,
    });
    expect(query('.openbitfun-agent-companion-window__bubble')).toBeNull();

    pushActivity({
      mood: 'rest',
      tasks: [task({ state: 'completed', labelKey: 'agentCompanion.activity.completed', defaultLabel: 'Completed' })],
      sequence: 3,
      emittedAt: 3,
    });
    expect(query('.openbitfun-agent-companion-window__bubble')).not.toBeNull();
  });
});
