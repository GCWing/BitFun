// @vitest-environment jsdom

import React, { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { SkillMarketItem } from '@/infrastructure/config/types';
import { useSkillMarket } from './useSkillMarket';

const listSkillMarketMock = vi.hoisted(() => vi.fn());
const searchSkillMarketMock = vi.hoisted(() => vi.fn());
const downloadSkillMarketMock = vi.hoisted(() => vi.fn());
const installedChangedMock = vi.hoisted(() => vi.fn());
const notificationMocks = vi.hoisted(() => ({
  success: vi.fn(),
  warning: vi.fn(),
  error: vi.fn(),
}));

vi.mock('react-i18next', () => ({
  useTranslation: () => ({ t: (key: string) => key }),
}));
vi.mock('@/infrastructure/api', () => ({
  configAPI: {
    listSkillMarket: listSkillMarketMock,
    searchSkillMarket: searchSkillMarketMock,
    downloadSkillMarket: downloadSkillMarketMock,
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

let currentMarket: ReturnType<typeof useSkillMarket> | null = null;

function Harness({ enabled }: { enabled: boolean }) {
  const market = useSkillMarket({
    searchQuery: '',
    installedSkillNames: new Set(),
    enabled,
    onInstalledChanged: installedChangedMock,
  });
  currentMarket = market;
  return <span>{market.marketLoading ? 'loading' : 'idle'}</span>;
}

describe('useSkillMarket', () => {
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);
    listSkillMarketMock.mockReset().mockResolvedValue([]);
    searchSkillMarketMock.mockReset().mockResolvedValue([]);
    downloadSkillMarketMock.mockReset();
    installedChangedMock.mockReset();
    notificationMocks.success.mockReset();
    notificationMocks.warning.mockReset();
    notificationMocks.error.mockReset();
    currentMarket = null;
  });

  afterEach(async () => {
    await act(async () => root.unmount());
    container.remove();
  });

  it('does not query the skill market outside the desktop app', async () => {
    await act(async () => {
      root.render(<Harness enabled={false} />);
      await Promise.resolve();
    });

    expect(listSkillMarketMock).not.toHaveBeenCalled();
    expect(searchSkillMarketMock).not.toHaveBeenCalled();
    expect(downloadSkillMarketMock).not.toHaveBeenCalled();
    expect(container.textContent).toBe('idle');

    await act(async () => {
      root.render(<Harness enabled />);
      await Promise.resolve();
    });

    expect(listSkillMarketMock).toHaveBeenCalledTimes(1);
  });

  it('ignores a market load that finishes after switching away', async () => {
    let resolveLoad: ((skills: SkillMarketItem[]) => void) | undefined;
    listSkillMarketMock.mockReturnValueOnce(new Promise<SkillMarketItem[]>((resolve) => {
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
        id: 'test',
        name: 'test',
        description: '',
        source: 'test',
        installs: 0,
        url: 'https://example.com/test',
        installId: 'test',
      }]);
      await Promise.resolve();
    });

    expect(currentMarket?.marketSkills).toEqual([]);
    expect(container.textContent).toBe('idle');
  });

  it('does not notify or reload after a pending download loses desktop capability', async () => {
    let resolveDownload: ((result: { installedSkills: string[] }) => void) | undefined;
    downloadSkillMarketMock.mockReturnValueOnce(new Promise((resolve) => {
      resolveDownload = resolve;
    }));
    const skill: SkillMarketItem = {
      id: 'test',
      name: 'test',
      description: '',
      source: 'test',
      installs: 0,
      url: 'https://example.com/test',
      installId: 'test',
    };

    await act(async () => {
      root.render(<Harness enabled />);
      await Promise.resolve();
    });
    let download: Promise<void> | undefined;
    await act(async () => {
      download = currentMarket?.handleDownload(skill, 'user');
      await Promise.resolve();
    });
    await act(async () => {
      root.render(<Harness enabled={false} />);
      await Promise.resolve();
    });
    await act(async () => {
      resolveDownload?.({ installedSkills: ['test'] });
      await download;
    });

    expect(notificationMocks.success).not.toHaveBeenCalled();
    expect(notificationMocks.error).not.toHaveBeenCalled();
    expect(installedChangedMock).not.toHaveBeenCalled();
  });
});
