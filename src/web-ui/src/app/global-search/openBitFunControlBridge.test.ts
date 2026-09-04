import { appearanceService } from '@/infrastructure/appearance';
import { configManager } from '@/infrastructure/config';
import { i18nService } from '@/infrastructure/i18n';
import { afterEach, describe, expect, it, vi } from 'vitest';
import {
  applyOpenBitFunControlEffect,
  executeOpenBitFunPresentationRequest,
} from './openBitFunControlBridge';

const mocks = vi.hoisted(() => ({
  activateInteractiveCapability: vi.fn(),
  activateProductAction: vi.fn(),
}));

vi.mock('./interactiveCapabilityActivator', () => ({
  activateInteractiveCapability: mocks.activateInteractiveCapability,
}));
vi.mock('./productActionActivator', () => ({
  activateProductAction: mocks.activateProductAction,
}));

describe('OpenBitFunControl presentation bridge', () => {
  afterEach(() => {
    vi.clearAllMocks();
    vi.restoreAllMocks();
  });

  it('opens the exact documented item selected by the native executor', async () => {
    await executeOpenBitFunPresentationRequest({
      requestId: 'open-shortcuts',
      action: 'open',
      capabilityId: 'setting.application.shortcuts',
      itemId: 'shortcut-browser',
    });
    expect(mocks.activateInteractiveCapability).toHaveBeenCalledWith(
      'setting.application.shortcuts',
      { itemId: 'shortcut-browser' },
    );
  });

  it('executes only a generated product-action binding', async () => {
    await executeOpenBitFunPresentationRequest({
      requestId: 'new-session',
      action: 'execute',
      capabilityId: 'feature.ai-assistant',
      operationId: 'new-session',
    });
    expect(mocks.activateProductAction).toHaveBeenCalledWith('session.new');
  });

  it('rejects discovery and mutation attempts at the presentation boundary', async () => {
    await expect(executeOpenBitFunPresentationRequest({
      requestId: 'get',
      action: 'get',
      capabilityId: 'feature.browser',
    })).rejects.toThrow('native ProductControl executor');
    await expect(executeOpenBitFunPresentationRequest({
      requestId: 'configure',
      action: 'configure',
      capabilityId: 'setting.application.appearance',
    })).rejects.toThrow('native ProductControl executor');
  });

  it('acknowledges a GUI-applied appearance without waiting on its own config mutation', async () => {
    vi.spyOn(appearanceService, 'getSnapshot').mockReturnValue({
      ...appearanceService.getSnapshot(),
      selectedAppearanceId: 'openbitfun-dark',
    });
    const reload = vi.spyOn(configManager, 'applyExternalReload').mockResolvedValue(undefined);
    const reconcile = vi.spyOn(appearanceService, 'reconcilePersistedState').mockResolvedValue(undefined);

    await expect(applyOpenBitFunControlEffect({
      capabilityId: 'setting.application.appearance',
      optionId: 'theme',
      changedPaths: ['appearance.selection'],
      value: 'openbitfun-dark',
    })).resolves.toEqual({ status: 'alreadyApplied' });
    expect(reload).not.toHaveBeenCalled();
    expect(reconcile).not.toHaveBeenCalled();
  });

  it('acknowledges a runtime-applied pending appearance before its config mutation completes', async () => {
    vi.spyOn(appearanceService, 'getSnapshot').mockReturnValue({
      ...appearanceService.getSnapshot(),
      status: 'applying',
      selectedAppearanceId: 'system',
      pendingSelectionId: 'openbitfun-dark',
    });
    const pendingApplied = vi.spyOn(appearanceService, 'hasAppliedPendingSelection')
      .mockReturnValue(true);
    const reload = vi.spyOn(configManager, 'applyExternalReload').mockResolvedValue(undefined);
    const reconcile = vi.spyOn(appearanceService, 'reconcilePersistedState').mockResolvedValue(undefined);

    await expect(applyOpenBitFunControlEffect({
      capabilityId: 'setting.application.appearance',
      optionId: 'theme',
      changedPaths: ['appearance.selection'],
      value: 'openbitfun-dark',
    })).resolves.toEqual({ status: 'alreadyApplied' });
    expect(pendingApplied).toHaveBeenCalledWith('openbitfun-dark');
    expect(reload).not.toHaveBeenCalled();
    expect(reconcile).not.toHaveBeenCalled();
  });

  it('applies an Agent appearance mutation through persisted-state reconciliation', async () => {
    vi.spyOn(appearanceService, 'getSnapshot').mockReturnValue({
      ...appearanceService.getSnapshot(),
      selectedAppearanceId: 'system',
    });
    const reload = vi.spyOn(configManager, 'applyExternalReload').mockResolvedValue(undefined);
    const reconcile = vi.spyOn(appearanceService, 'reconcilePersistedState').mockResolvedValue(undefined);

    await expect(applyOpenBitFunControlEffect({
      capabilityId: 'setting.application.appearance',
      optionId: 'theme',
      changedPaths: ['appearance.selection'],
      value: 'openbitfun-tokyo-night',
    })).resolves.toEqual({ status: 'applied' });
    expect(reload).toHaveBeenCalledOnce();
    expect(reconcile).toHaveBeenCalledOnce();
  });

  it('applies an Agent language mutation without persisting it a second time', async () => {
    vi.spyOn(i18nService, 'getCurrentLocale').mockReturnValue('zh-CN');
    vi.spyOn(configManager, 'applyExternalReload').mockResolvedValue(undefined);
    const applyLanguage = vi.spyOn(i18nService, 'applyPersistedLanguage').mockResolvedValue(undefined);

    await applyOpenBitFunControlEffect({
      capabilityId: 'setting.application.appearance',
      optionId: 'language',
      changedPaths: ['app.language'],
      value: 'en-US',
    });
    expect(applyLanguage).toHaveBeenCalledWith('en-US');
  });

  it('reconciles the previous appearance when the native transaction rolls back', async () => {
    vi.spyOn(appearanceService, 'getSnapshot').mockReturnValue({
      ...appearanceService.getSnapshot(),
      selectedAppearanceId: 'openbitfun-dark',
    });
    const reload = vi.spyOn(configManager, 'applyExternalReload').mockResolvedValue(undefined);
    const reconcile = vi.spyOn(appearanceService, 'reconcilePersistedState').mockResolvedValue(undefined);

    await applyOpenBitFunControlEffect({
      capabilityId: 'setting.application.appearance',
      optionId: 'theme',
      changedPaths: ['appearance.selection'],
      value: 'system',
    });

    expect(reload).toHaveBeenCalledOnce();
    expect(reconcile).toHaveBeenCalledOnce();
  });

  it('rejects a required runtime effect so the native executor can roll back', async () => {
    vi.spyOn(i18nService, 'getCurrentLocale').mockReturnValue('zh-CN');
    vi.spyOn(configManager, 'applyExternalReload').mockResolvedValue(undefined);
    vi.spyOn(i18nService, 'applyPersistedLanguage').mockRejectedValue(
      new Error('renderer locale failed'),
    );

    await expect(applyOpenBitFunControlEffect({
      capabilityId: 'setting.application.appearance',
      optionId: 'language',
      changedPaths: ['app.language'],
      value: 'en-US',
    })).rejects.toThrow('renderer locale failed');
  });
});
