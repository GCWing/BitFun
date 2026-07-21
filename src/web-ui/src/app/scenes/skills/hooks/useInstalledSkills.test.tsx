// @vitest-environment jsdom

import React, { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { SkillInfo } from '@/infrastructure/config/types';
import { useInstalledSkills } from './useInstalledSkills';

const getSkillConfigsMock = vi.hoisted(() => vi.fn());
const deleteSkillMock = vi.hoisted(() => vi.fn());
const validateSkillPathMock = vi.hoisted(() => vi.fn());
const notificationMocks = vi.hoisted(() => ({
  success: vi.fn(),
  warning: vi.fn(),
  error: vi.fn(),
}));

vi.mock('@tauri-apps/plugin-dialog', () => ({ open: vi.fn() }));
vi.mock('react-i18next', () => ({
  useTranslation: () => ({ t: (key: string) => key }),
}));
vi.mock('@/infrastructure/api', () => ({
  configAPI: {
    getSkillConfigs: getSkillConfigsMock,
    validateSkillPath: validateSkillPathMock,
    addSkill: vi.fn(),
    deleteSkill: deleteSkillMock,
  },
}));
vi.mock('@/infrastructure/hooks/useWorkspaceManagerSync', () => ({
  useWorkspaceManagerSync: () => ({
    workspacePath: 'D:/workspace/project',
    hasWorkspace: true,
    isRemoteWorkspace: false,
  }),
}));
vi.mock('@/shared/notification-system', () => ({
  useNotification: () => notificationMocks,
}));

let currentInstalled: ReturnType<typeof useInstalledSkills> | null = null;

function Harness({ enabled }: { enabled: boolean }) {
  const installed = useInstalledSkills({
    searchQuery: '',
    activeFilter: 'all',
    enabled,
  });
  currentInstalled = installed;
  return <span>{installed.loading ? 'loading' : 'idle'}</span>;
}

describe('useInstalledSkills', () => {
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);
    getSkillConfigsMock.mockReset().mockResolvedValue([]);
    deleteSkillMock.mockReset().mockResolvedValue(undefined);
    validateSkillPathMock.mockReset().mockResolvedValue({ valid: true, name: 'test' });
    notificationMocks.success.mockReset();
    notificationMocks.warning.mockReset();
    notificationMocks.error.mockReset();
    currentInstalled = null;
  });

  afterEach(async () => {
    await act(async () => root.unmount());
    container.remove();
    vi.useRealTimers();
  });

  it('does not query desktop skill configuration during a remote connection', async () => {
    await act(async () => {
      root.render(<Harness enabled={false} />);
      await Promise.resolve();
    });

    expect(getSkillConfigsMock).not.toHaveBeenCalled();
    expect(container.textContent).toBe('idle');

    await act(async () => {
      root.render(<Harness enabled />);
      await Promise.resolve();
    });

    expect(getSkillConfigsMock).toHaveBeenCalledTimes(1);
  });

  it('ignores a desktop skill load that finishes after switching away', async () => {
    let resolveLoad: ((skills: SkillInfo[]) => void) | undefined;
    getSkillConfigsMock.mockReturnValueOnce(new Promise<SkillInfo[]>((resolve) => {
      resolveLoad = resolve;
    }));

    await act(async () => {
      root.render(<Harness enabled />);
      await Promise.resolve();
    });
    await act(async () => {
      root.render(<Harness enabled={false} />);
      await Promise.resolve();
    });
    await act(async () => {
      resolveLoad?.([{
        key: 'bitfun:user:test',
        name: 'test',
        description: '',
        path: 'D:/skills/test',
        level: 'user',
        sourceSlot: 'bitfun-user',
        sourceId: 'bitfun',
        dirName: 'test',
        isBuiltin: false,
      }]);
      await Promise.resolve();
    });

    expect(currentInstalled?.skills).toEqual([]);
    expect(container.textContent).toBe('idle');
  });

  it('does not notify or reload after a pending delete loses desktop capability', async () => {
    let resolveDelete: (() => void) | undefined;
    deleteSkillMock.mockReturnValueOnce(new Promise<void>((resolve) => {
      resolveDelete = resolve;
    }));
    const skill: SkillInfo = {
      key: 'bitfun:user:test',
      name: 'test',
      description: '',
      path: 'D:/skills/test',
      level: 'user',
      sourceSlot: 'bitfun-user',
      sourceId: 'bitfun',
      dirName: 'test',
      isBuiltin: false,
    };

    await act(async () => {
      root.render(<Harness enabled />);
      await Promise.resolve();
    });
    let deletion: Promise<boolean> | undefined;
    await act(async () => {
      deletion = currentInstalled?.handleDelete(skill);
      await Promise.resolve();
    });
    await act(async () => {
      root.render(<Harness enabled={false} />);
      await Promise.resolve();
    });
    await act(async () => {
      resolveDelete?.();
      await deletion;
    });

    expect(notificationMocks.success).not.toHaveBeenCalled();
    expect(notificationMocks.error).not.toHaveBeenCalled();
    expect(getSkillConfigsMock).toHaveBeenCalledTimes(1);
  });

  it('ignores path validation that finishes after switching away', async () => {
    vi.useFakeTimers();
    let resolveValidation: ((result: { valid: boolean; name: string }) => void) | undefined;
    validateSkillPathMock.mockReturnValueOnce(new Promise((resolve) => {
      resolveValidation = resolve;
    }));

    await act(async () => {
      root.render(<Harness enabled />);
      await Promise.resolve();
    });
    await act(async () => {
      currentInstalled?.setFormPath('D:/skills/test');
      await Promise.resolve();
    });
    await act(async () => {
      await vi.advanceTimersByTimeAsync(300);
    });
    expect(validateSkillPathMock).toHaveBeenCalledTimes(1);

    await act(async () => {
      root.render(<Harness enabled={false} />);
      await Promise.resolve();
    });
    await act(async () => {
      resolveValidation?.({ valid: true, name: 'test' });
      await Promise.resolve();
    });

    expect(currentInstalled?.validationResult).toBeNull();
    expect(currentInstalled?.isValidating).toBe(false);
  });
});
