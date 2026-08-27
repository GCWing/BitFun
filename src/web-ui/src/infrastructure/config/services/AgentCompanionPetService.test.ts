import { beforeEach, describe, expect, it, vi } from 'vitest';

const invoke = vi.fn();

vi.mock('@/infrastructure/api/service-api/ApiClient', () => ({
  api: { invoke },
}));

vi.mock('@tauri-apps/plugin-fs', () => ({
  readFile: vi.fn(),
}));

vi.mock('@/infrastructure/runtime', () => ({
  isTauriRuntime: () => false,
}));

describe('AgentCompanionPetService built-in presets', () => {
  beforeEach(() => {
    invoke.mockReset();
  });

  it('defaults to the blue-golden cat while retaining the previous Bitfun preset', async () => {
    const { DEFAULT_AGENT_COMPANION_PET, listAgentCompanionPets } = await import('./AgentCompanionPetService');

    const pets = await listAgentCompanionPets();
    const blueGolden = pets.find(pet => pet.id === 'blue-golden');
    const bitfun = pets.find(pet => pet.id === 'bitfun');

    expect(DEFAULT_AGENT_COMPANION_PET).toMatchObject({
      id: 'blue-golden',
      displayName: '困困',
      source: 'preset',
      packagePath: '/agent-companion-pets/blue-golden',
      spritesheetPath: '/agent-companion-pets/blue-golden/spritesheet.png',
      spritesheetMimeType: 'image/png',
    });
    expect(blueGolden).toMatchObject({
      ...DEFAULT_AGENT_COMPANION_PET,
      previewSrc: '/agent-companion-pets/blue-golden/spritesheet.png',
    });
    expect(pets[0]).toMatchObject(DEFAULT_AGENT_COMPANION_PET);
    expect(bitfun).toMatchObject({
      displayName: 'Bitfun',
      packagePath: '/agent-companion-pets/bitfun',
      spritesheetPath: '/agent-companion-pets/bitfun/spritesheet.webp',
    });
    expect(invoke).not.toHaveBeenCalled();
  });
});
