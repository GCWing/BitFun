import { beforeEach, describe, expect, it, vi } from 'vitest';

const configManagerMock = vi.hoisted(() => ({
  getConfig: vi.fn(),
  setConfig: vi.fn(),
  watch: vi.fn(),
}));

const configApiMock = vi.hoisted(() => ({
  getConfig: vi.fn(),
}));

vi.mock('./ConfigManager', () => ({
  configManager: configManagerMock,
}));

vi.mock('@/infrastructure/api/service-api/ConfigAPI', () => ({
  configAPI: configApiMock,
}));

vi.mock('./AgentCompanionPetService', () => ({
  DEFAULT_AGENT_COMPANION_PET: {
    id: 'default',
    displayName: 'Default',
    source: 'preset',
    packagePath: '',
    spritesheetPath: '',
    spritesheetMimeType: 'image/png',
  },
}));

vi.mock('@/shared/utils/logger', () => ({
  createLogger: () => ({
    warn: vi.fn(),
    error: vi.fn(),
  }),
}));

describe('AIExperienceConfigService startup behavior', () => {
  beforeEach(() => {
    vi.resetModules();
    vi.clearAllMocks();
    configManagerMock.watch.mockReturnValue(() => undefined);
  });

  it('does not read app.ai_experience during module import', async () => {
    await import('./AIExperienceConfigService');
    await Promise.resolve();

    expect(configManagerMock.getConfig).not.toHaveBeenCalled();
  });

  it('loads settings lazily when requested', async () => {
    configManagerMock.getConfig.mockResolvedValueOnce({
      enable_agent_companion: false,
    });
    const { aiExperienceConfigService } = await import('./AIExperienceConfigService');

    await aiExperienceConfigService.getSettingsAsync();

    expect(configManagerMock.watch).toHaveBeenCalledTimes(1);
    expect(configManagerMock.watch).toHaveBeenCalledWith('app.ai_experience', expect.any(Function));
    expect(configManagerMock.getConfig).toHaveBeenCalledTimes(1);
    expect(configManagerMock.getConfig).toHaveBeenCalledWith('app.ai_experience');
  });

  it('can force refresh settings for cross-window lifecycle synchronization', async () => {
    configApiMock.getConfig.mockResolvedValueOnce({
      enable_agent_companion: true,
      agent_companion_display_mode: 'desktop',
    });
    const { aiExperienceConfigService } = await import('./AIExperienceConfigService');

    await aiExperienceConfigService.getSettingsAsync({ forceRefresh: true });

    expect(configApiMock.getConfig).toHaveBeenCalledWith('app.ai_experience');
    expect(configManagerMock.getConfig).not.toHaveBeenCalled();
  });
});
