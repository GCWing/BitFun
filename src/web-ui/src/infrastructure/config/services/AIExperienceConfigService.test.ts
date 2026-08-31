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
    id: 'blue-golden',
    displayName: '困困',
    source: 'preset',
    packagePath: '/agent-companion-pets/blue-golden',
    spritesheetPath: '/agent-companion-pets/blue-golden/spritesheet.png',
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

  it('uses the blue-golden cat when no companion pet has been configured', async () => {
    configManagerMock.getConfig.mockResolvedValueOnce({
      enable_agent_companion: true,
    });
    const { aiExperienceConfigService } = await import('./AIExperienceConfigService');

    const settings = await aiExperienceConfigService.getSettingsAsync();

    expect(settings.agent_companion_pet).toMatchObject({
      id: 'blue-golden',
      displayName: '困困',
      packagePath: '/agent-companion-pets/blue-golden',
      spritesheetPath: '/agent-companion-pets/blue-golden/spritesheet.png',
    });
  });

  it('preserves an existing user-selected companion pet', async () => {
    const selectedPet = {
      id: 'usagi',
      displayName: 'Usagi',
      source: 'preset' as const,
      packagePath: '/agent-companion-pets/usagi',
      spritesheetPath: '/agent-companion-pets/usagi/spritesheet.webp',
      spritesheetMimeType: 'image/webp',
    };
    configManagerMock.getConfig.mockResolvedValueOnce({
      enable_agent_companion: false,
      agent_companion_pet: selectedPet,
    });
    const { aiExperienceConfigService } = await import('./AIExperienceConfigService');

    const settings = await aiExperienceConfigService.getSettingsAsync();

    expect(settings.agent_companion_pet).toEqual(selectedPet);
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

  it('does not reset cloud voice input when a stale settings view toggles another feature', async () => {
    const persisted = {
      enable_agent_companion: true,
      voice_input: { provider: 'cloud', model_id: 'cloud-fixture', microphone_device_id: 'saved-mic' },
      quick_actions: [{ id: 'fixture', label: 'Fixture', prompt: 'fixture', enabled: true }],
    };
    configManagerMock.getConfig.mockResolvedValue(persisted);
    configManagerMock.setConfig.mockImplementation(async (path: string, value: unknown) => {
      expect(path).toBe('app.ai_experience.enable_agent_companion');
      persisted.enable_agent_companion = value as boolean;
    });
    const { aiExperienceConfigService } = await import('./AIExperienceConfigService');

    // No preceding read is required; fallback defaults must never be saved.
    await aiExperienceConfigService.saveSettings({ enable_agent_companion: false });
    await aiExperienceConfigService.reload();
    expect(aiExperienceConfigService.getSettings()).toMatchObject({
      enable_agent_companion: false,
      voice_input: persisted.voice_input,
      quick_actions: persisted.quick_actions,
    });
    expect(configManagerMock.setConfig).toHaveBeenCalledTimes(1);
  });

  it('updates only an edited voice input field and preserves an explicitly empty quick action list', async () => {
    configManagerMock.getConfig.mockResolvedValue({ quick_actions: [] });
    configManagerMock.setConfig.mockResolvedValue(undefined);
    const { aiExperienceConfigService } = await import('./AIExperienceConfigService');

    await aiExperienceConfigService.saveSettings({ voice_input: { microphone_device_id: 'new-mic' } });
    expect(configManagerMock.setConfig).toHaveBeenCalledWith(
      'app.ai_experience.voice_input.microphone_device_id', 'new-mic'
    );
    await aiExperienceConfigService.reload();
    expect(aiExperienceConfigService.getSettings().quick_actions).toEqual([]);
  });
});
