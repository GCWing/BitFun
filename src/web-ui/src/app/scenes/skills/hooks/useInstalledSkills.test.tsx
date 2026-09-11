// @vitest-environment jsdom

import React, { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { SkillInfo } from '@/infrastructure/config/types';
import { useInstalledSkills } from './useInstalledSkills';
import type { InstalledFilter } from '../skillsSceneStore';

const getSkillConfigsMock = vi.hoisted(() => vi.fn());
const translateMock = vi.hoisted(() => (key: string) => key);
const diagnosticsMock = vi.hoisted(() => ({
  items: [] as Array<{path: string; sourceId: string; message: string}>,
  available: true,
}));
const getGlobalSkillSettingsMock = vi.hoisted(() => vi.fn());
const setGlobalSkillDisabledMock = vi.hoisted(() => vi.fn());
const deleteSkillMock = vi.hoisted(() => vi.fn());
const validateSkillPathMock = vi.hoisted(() => vi.fn());
const notificationMocks = vi.hoisted(() => ({
  success: vi.fn(),
  warning: vi.fn(),
  info: vi.fn(),
  error: vi.fn(),
}));

vi.mock('@tauri-apps/plugin-dialog', () => ({ open: vi.fn() }));
vi.mock('react-i18next', () => ({
  useTranslation: () => ({ t: translateMock }),
}));
vi.mock('@/infrastructure/api', () => ({
  configAPI: {
    getSkillScanReport: async (...args: unknown[]) => ({ skills: await getSkillConfigsMock(...args), diagnostics: diagnosticsMock.items, diagnosticsAvailable: diagnosticsMock.available }),
    getGlobalSkillSettings: getGlobalSkillSettingsMock,
    setGlobalSkillDisabled: setGlobalSkillDisabledMock,
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

function Harness({ enabled, activeFilter = 'all', searchQuery = '' }: {
  enabled: boolean;
  activeFilter?: InstalledFilter;
  searchQuery?: string;
}) {
  const installed = useInstalledSkills({
    searchQuery,
    activeFilter,
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
    getGlobalSkillSettingsMock.mockReset().mockResolvedValue({
      globallyDisabledUserSkillKeys: [],
    });
    setGlobalSkillDisabledMock.mockReset().mockResolvedValue({
      globallyDisabledUserSkillKeys: [],
    });
    deleteSkillMock.mockReset().mockResolvedValue(undefined);
    validateSkillPathMock.mockReset().mockResolvedValue({ valid: true, name: 'test' });
    notificationMocks.success.mockReset();
    notificationMocks.warning.mockReset();
    notificationMocks.info.mockReset();
    notificationMocks.error.mockReset();
    currentInstalled = null;
    diagnosticsMock.items = [];
    diagnosticsMock.available = true;
  });

  afterEach(async () => {
    await act(async () => root.unmount());
    container.remove();
    vi.useRealTimers();
  });

  it('preserves available skills alongside discovery failures', async () => {
    const skills = [{ key: 'user::codex::good', name: 'good', sourceId: 'codex', level: 'user', path: '/skills/good' }];
    getSkillConfigsMock.mockResolvedValue(skills);
    diagnosticsMock.items = [{ path: '/skills/bad/SKILL.md', sourceId: 'codex', message: 'missing description' }];
    await act(async () => root.render(<Harness enabled />));
    expect(currentInstalled?.skills).toEqual(skills);
    expect(currentInstalled?.diagnostics).toEqual(diagnosticsMock.items);
    expect(currentInstalled?.error).toBeNull();
    expect(notificationMocks.warning).toHaveBeenCalledExactlyOnceWith('list.scanIncomplete', {
      title: 'nav.title',
      metadata: { diagnostics: '/skills/bad/SKILL.md: missing description' },
    });
  });

  it('does not repeat unchanged scan warnings on refresh, but reports changes and recurrence', async () => {
    diagnosticsMock.items = [
      { path: '/skills/first', sourceId: 'codex', message: 'missing target' },
      { path: '/skills/second', sourceId: 'opencode', message: 'permission denied' },
    ];
    await act(async () => root.render(<Harness enabled />));
    expect(notificationMocks.warning).toHaveBeenCalledTimes(1);

    diagnosticsMock.items = [...diagnosticsMock.items].reverse();
    await act(async () => { await currentInstalled?.loadSkills(true); });
    expect(notificationMocks.warning).toHaveBeenCalledTimes(1);

    diagnosticsMock.items = [{ ...diagnosticsMock.items[0], message: 'missing target' }];
    await act(async () => { await currentInstalled?.loadSkills(true); });
    expect(notificationMocks.warning).toHaveBeenCalledTimes(2);

    const failures = diagnosticsMock.items;
    diagnosticsMock.items = [];
    await act(async () => { await currentInstalled?.loadSkills(true); });
    expect(notificationMocks.warning).toHaveBeenCalledTimes(2);
    diagnosticsMock.items = failures;
    await act(async () => { await currentInstalled?.loadSkills(true); });
    expect(notificationMocks.warning).toHaveBeenCalledTimes(3);
  });

  it('does not claim skills are available when every scanned skill failed', async () => {
    diagnosticsMock.items = [{ path: '/skills/broken', sourceId: 'opencode', message: 'missing target' }];
    await act(async () => root.render(<Harness enabled />));
    expect(currentInstalled?.skills).toEqual([]);
    expect(notificationMocks.warning).toHaveBeenCalledExactlyOnceWith('list.loadFailed', {
      title: 'nav.title',
      metadata: { diagnostics: '/skills/broken: missing target' },
    });
  });

  it('reports hosts without scan diagnostics once as informational feedback', async () => {
    diagnosticsMock.available = false;
    await act(async () => root.render(<Harness enabled />));
    await act(async () => { await currentInstalled?.loadSkills(true); });
    expect(currentInstalled?.diagnosticsAvailable).toBe(false);
    expect(notificationMocks.warning).not.toHaveBeenCalled();
    expect(notificationMocks.info).toHaveBeenCalledExactlyOnceWith('list.diagnosticsUnavailable', {
      title: 'nav.title',
    });
  });

  it('does not query desktop skill configuration during a remote connection' , async () => {
    await act(async () => {
      root.render(<Harness enabled={false} />);
      await Promise.resolve();
    });

    expect(getSkillConfigsMock).not.toHaveBeenCalled();
    expect(getGlobalSkillSettingsMock).not.toHaveBeenCalled();
    expect(container.textContent).toBe('idle');

    await act(async () => {
      root.render(<Harness enabled />);
      await Promise.resolve();
    });

    expect(getSkillConfigsMock).toHaveBeenCalledTimes(1);
    expect(getGlobalSkillSettingsMock).toHaveBeenCalledTimes(1);
  });

  it('groups external agents across scopes and keeps counts independent of search', async () => {
    const skill = (key: string, overrides: Partial<SkillInfo> = {}): SkillInfo => ({
      key, name: 'shared-name', description: '', path: `/skills/${key}`,
      level: 'user', sourceSlot: 'openbitfun', sourceId: 'openbitfun',
      dirName: 'shared-name', isBuiltin: false, ...overrides,
    });
    const skills = [
      skill('owned-user'),
      skill('owned-project', { level: 'project' }),
      skill('builtin', { isBuiltin: true }),
      skill('codex-user', { sourceId: 'codex', sourceSlot: 'home.codex' }),
      skill('codex-project', { sourceId: '', sourceSlot: 'codex', level: 'project', description: 'remote workspace' }),
      skill('claude', { sourceId: 'claude-code', sourceSlot: 'home.claude', isShadowed: true }),
      skill('agents', { sourceId: 'agent-skills', sourceSlot: 'home.agents' }),
    ];
    getSkillConfigsMock.mockResolvedValue(skills);
    await act(async () => root.render(<Harness enabled activeFilter="source:codex" />));
    expect(currentInstalled?.filteredSkills.map((item) => item.key)).toEqual(['codex-user', 'codex-project']);
    expect(currentInstalled?.sourceGroups).toEqual([
      { id: 'source:agent-skills', label: 'Agent Skills' },
      { id: 'source:claude-code', label: 'Claude Code' },
      { id: 'source:codex', label: 'Codex' },
    ]);
    expect(currentInstalled?.counts).toEqual({
      all: 7, builtin: 1, user: 1, project: 1,
      'source:codex': 2, 'source:claude-code': 1, 'source:agent-skills': 1,
    });
    await act(async () => root.render(<Harness enabled activeFilter="source:codex" searchQuery="remote" />));
    expect(currentInstalled?.filteredSkills.map((item) => item.key)).toEqual(['codex-project']);
    expect(currentInstalled?.counts['source:codex']).toBe(2);
    await act(async () => root.render(<Harness enabled activeFilter="user" />));
    expect(currentInstalled?.filteredSkills.map((item) => item.key)).toEqual(['owned-user']);
    await act(async () => root.render(<Harness enabled activeFilter="project" />));
    expect(currentInstalled?.filteredSkills.map((item) => item.key)).toEqual(['owned-project']);
    await act(async () => root.render(<Harness enabled activeFilter="all" />));
    expect(currentInstalled?.filteredSkills).toEqual(skills);
  });

  it('ignores a desktop skill load that finishes after switching away', async () => {
    diagnosticsMock.items = [{ path: '/skills/broken', sourceId: 'opencode', message: 'missing target' }];
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
        key: 'openbitfun:user:test',
        name: 'test',
        description: '',
        path: 'D:/skills/test',
        level: 'user',
        sourceSlot: 'openbitfun-user',
        sourceId: 'openbitfun',
        dirName: 'test',
        isBuiltin: false,
      }]);
      await Promise.resolve();
    });

    expect(currentInstalled?.skills).toEqual([]);
    expect(container.textContent).toBe('idle');
    expect(notificationMocks.warning).not.toHaveBeenCalled();
    expect(notificationMocks.info).not.toHaveBeenCalled();
  });

  it('does not notify or reload after a pending delete loses desktop capability', async () => {
    let resolveDelete: (() => void) | undefined;
    deleteSkillMock.mockReturnValueOnce(new Promise<void>((resolve) => {
      resolveDelete = resolve;
    }));
    const skill: SkillInfo = {
      key: 'openbitfun:user:test',
      name: 'test',
      description: '',
      path: 'D:/skills/test',
      level: 'user',
      sourceSlot: 'openbitfun-user',
      sourceId: 'openbitfun',
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
