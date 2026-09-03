import { afterEach, describe, expect, it, vi } from 'vitest';
import {
  abandonSettingsDraftsForContextSwitch,
  cancelPendingSettingsNavigation,
  discardAndContinueSettingsNavigation,
  getSettingsDraftSnapshot,
  registerSettingsDraft,
  requestSettingsDraftExit,
  requestSettingsNavigation,
  resetSettingsDraftRegistryForTests,
  saveAndContinueSettingsNavigation,
  updateSettingsDraft,
} from './settingsDraftRegistry';

describe('settingsDraftRegistry', () => {
  afterEach(() => resetSettingsDraftRegistryForTests());

  it('navigates immediately when the active resource remains in scope', () => {
    const commit = vi.fn();
    registerSettingsDraft({
      id: 'page-form',
      pageId: 'application.editor',
      label: 'Editor',
      dirty: true,
      save: vi.fn(),
      discard: vi.fn(),
    });

    expect(requestSettingsNavigation(
      { pageId: 'application.editor' },
      { kind: 'settings', pageId: 'application.editor', viewId: 'advanced' },
      commit,
    )).toBe(true);
    expect(commit).toHaveBeenCalledOnce();
    expect(getSettingsDraftSnapshot().pendingNavigation).toBeNull();
  });

  it('blocks a view-scoped draft and saves before committing navigation', async () => {
    const commit = vi.fn();
    const save = vi.fn(async () => true);
    registerSettingsDraft({
      id: 'acp-json',
      pageId: 'tools.acp',
      viewId: 'json',
      label: 'ACP JSON',
      dirty: true,
      save,
      discard: vi.fn(),
    });

    expect(requestSettingsNavigation(
      { pageId: 'tools.acp', viewId: 'json' },
      { kind: 'settings', pageId: 'tools.acp', viewId: 'local' },
      commit,
    )).toBe(false);
    expect(getSettingsDraftSnapshot().pendingNavigation?.resourceLabels).toEqual(['ACP JSON']);

    await expect(saveAndContinueSettingsNavigation()).resolves.toBe(true);
    expect(save).toHaveBeenCalledOnce();
    expect(commit).toHaveBeenCalledOnce();
  });

  it('keeps the decision open when saving fails', async () => {
    const commit = vi.fn();
    registerSettingsDraft({
      id: 'invalid-form',
      pageId: 'workspace.worktrees',
      label: 'Worktrees',
      dirty: true,
      save: async () => false,
      discard: vi.fn(),
    });
    requestSettingsNavigation(
      { pageId: 'workspace.worktrees' },
      { kind: 'settings', pageId: 'application.general' },
      commit,
    );

    await expect(saveAndContinueSettingsNavigation()).resolves.toBe(false);
    expect(commit).not.toHaveBeenCalled();
    expect(getSettingsDraftSnapshot().pendingNavigation?.failed).toBe(true);
    cancelPendingSettingsNavigation();
  });

  it('can discard a page-owned editor before closing it', async () => {
    const commit = vi.fn();
    const discard = vi.fn();
    registerSettingsDraft({
      id: 'modal',
      pageId: 'ai.models',
      label: 'Model editor',
      dirty: true,
      save: vi.fn(),
      discard,
    });

    expect(requestSettingsDraftExit(['modal'], commit)).toBe(false);
    await expect(discardAndContinueSettingsNavigation()).resolves.toBe(true);
    expect(discard).toHaveBeenCalledOnce();
    expect(commit).toHaveBeenCalledOnce();
  });

  it('waits for an in-flight save and does not submit the same resource twice', async () => {
    const commit = vi.fn();
    const save = vi.fn();
    registerSettingsDraft({
      id: 'saving-form',
      pageId: 'ai.models',
      label: 'Proxy',
      dirty: true,
      saving: true,
      save,
      discard: vi.fn(),
    });
    requestSettingsNavigation(
      { pageId: 'ai.models' },
      { kind: 'settings', pageId: 'application.general' },
      commit,
    );

    const continuation = saveAndContinueSettingsNavigation();
    await Promise.resolve();
    expect(save).not.toHaveBeenCalled();
    expect(commit).not.toHaveBeenCalled();

    updateSettingsDraft('saving-form', { saving: false, dirty: false });
    await expect(continuation).resolves.toBe(true);
    expect(save).not.toHaveBeenCalled();
    expect(commit).toHaveBeenCalledOnce();
  });

  it('does not report a completed navigation when a context switch abandons an in-flight save', async () => {
    let finishSave: (() => void) | undefined;
    const save = vi.fn(() => new Promise<void>(resolve => {
      finishSave = resolve;
    }));
    const commit = vi.fn();
    registerSettingsDraft({
      id: 'old-device-form',
      pageId: 'application.voice',
      label: 'Voice',
      dirty: true,
      save,
      discard: vi.fn(),
    });
    requestSettingsNavigation(
      { pageId: 'application.voice' },
      null,
      commit,
    );

    const continuation = saveAndContinueSettingsNavigation();
    await vi.waitFor(() => expect(save).toHaveBeenCalledOnce());
    abandonSettingsDraftsForContextSwitch();
    finishSave?.();

    await expect(continuation).resolves.toBe(false);
    expect(commit).not.toHaveBeenCalled();
  });

  it('retries only resources that remain dirty after a partial multi-resource save', async () => {
    const commit = vi.fn();
    const saveFirst = vi.fn(() => {
      updateSettingsDraft('first', { dirty: false });
      return true;
    });
    const saveSecond = vi.fn()
      .mockResolvedValueOnce(false)
      .mockImplementationOnce(async () => {
        updateSettingsDraft('second', { dirty: false });
        return true;
      });
    registerSettingsDraft({
      id: 'first',
      pageId: 'tools.webSearch',
      label: 'Configuration',
      dirty: true,
      save: saveFirst,
      discard: vi.fn(),
    });
    registerSettingsDraft({
      id: 'second',
      pageId: 'tools.webSearch',
      label: 'Credential',
      dirty: true,
      save: saveSecond,
      discard: vi.fn(),
    });
    requestSettingsNavigation(
      { pageId: 'tools.webSearch' },
      { kind: 'settings', pageId: 'tools.mcp' },
      commit,
    );

    await expect(saveAndContinueSettingsNavigation()).resolves.toBe(false);
    expect(getSettingsDraftSnapshot().pendingNavigation?.resourceLabels).toEqual([
      'Credential',
    ]);
    await expect(saveAndContinueSettingsNavigation()).resolves.toBe(true);

    expect(saveFirst).toHaveBeenCalledOnce();
    expect(saveSecond).toHaveBeenCalledTimes(2);
    expect(commit).toHaveBeenCalledOnce();
  });

  it('abandons stale drafts and pending commits when the owning device changes', () => {
    const commit = vi.fn();
    registerSettingsDraft({
      id: 'old-device-form',
      pageId: 'application.voice',
      label: 'Voice',
      dirty: true,
      save: vi.fn(),
      discard: vi.fn(),
    });
    requestSettingsNavigation(
      { pageId: 'application.voice' },
      null,
      commit,
    );

    abandonSettingsDraftsForContextSwitch();

    expect(getSettingsDraftSnapshot().resources).toEqual([]);
    expect(getSettingsDraftSnapshot().pendingNavigation).toBeNull();
    expect(commit).not.toHaveBeenCalled();
  });
});
