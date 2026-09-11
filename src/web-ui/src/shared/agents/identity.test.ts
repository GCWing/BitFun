import { describe, expect, it } from 'vitest';
import { HARNESS_IDS, canonicalAgentId, canonicalAgentConfigId, primaryAgentKind, supportsSkillConfiguration } from './identity';
import { translateAgentIdentityCommand, translateAgentIdentityResponse } from '../../../../shared/agent-harness/wire';
import wireFixtures from '../../../../shared/agent-harness/wire-fixtures.json';

describe('Agent identity and capability contracts', () => {
  it('matches the Rust request and response compatibility fixtures', () => {
    for (const fixture of wireFixtures) {
      expect(translateAgentIdentityCommand(fixture.command, fixture.canonical, 'legacy')).toEqual(fixture.legacy);
      expect(translateAgentIdentityCommand(fixture.command, fixture.legacy, 'canonical')).toEqual(fixture.canonical);
      if ('canonicalResponse' in fixture) {
        expect(translateAgentIdentityResponse(fixture.command, fixture.canonicalResponse, fixture.canonical, 'legacy')).toEqual(fixture.legacyResponse);
        expect(translateAgentIdentityResponse(fixture.command, fixture.legacyResponse, fixture.legacy, 'canonical')).toEqual(fixture.canonicalResponse);
      }
    }
  });

  it('rejects conflicting profile aliases before sending a configuration update', () => {
    const request = { request: { path: 'ai.agent_profiles', value: {
      agentic: { removed_tools: ['Bash'] }, Standard: { removed_tools: ['Read'] },
    } } };
    const original = structuredClone(request);
    expect(() => translateAgentIdentityCommand('set_config', request, 'canonical')).toThrow('Conflicting Agent profile aliases');
    expect(request).toEqual(original);
  });

  it('has four canonical Harness identities and migrates legacy input only', () => {
    expect(HARNESS_IDS).toEqual(['Minimal', 'Standard', 'Ultimate', 'Creative']);
    for (const [legacy, id] of [['minimal', 'Minimal'], ['balanced', 'Standard'], ['agentic', 'Standard'], ['Ultra', 'Ultimate'], ['creative', 'Creative']]) {
      expect(canonicalAgentId(legacy)).toBe(id);
      expect(canonicalAgentId(id)).toBe(id);
    }
    expect(canonicalAgentConfigId('coding_shared')).toBe('Standard');
    expect(canonicalAgentId('external::Ultra')).toBe('external::Ultra');
    expect(primaryAgentKind({ id: 'Standard', source: 'external' })).toBe('agent');
    expect(primaryAgentKind({ id: 'Standard', source: 'builtin' })).toBe('harness');
    expect(primaryAgentKind({ id: 'Claw', source: 'builtin' })).toBe('agent');
  });

  it('offers skill configuration only for real, locally managed Skill capability', () => {
    expect(supportsSkillConfiguration({ defaultTools: ['Bash', 'Read'] })).toBe(false);
    expect(supportsSkillConfiguration({ defaultTools: ['Skill', 'Task'] })).toBe(true);
    expect(supportsSkillConfiguration({ defaultTools: ['Skill'], source: 'user' })).toBe(true);
    expect(supportsSkillConfiguration({ defaultTools: ['Skill'], source: 'external' })).toBe(false);
    expect(supportsSkillConfiguration({ defaultTools: ['Skill'], visibility: { showInGlobalRegistry: false } })).toBe(false);
  });

  it('preserves custom IDs and user content across the legacy peer dialect', () => {
    const input = { request: { agentType: 'Standard', modeId: 'Ultimate', arguments: { agentType: 'Standard' }, userMessageMetadata: { agentType: 'Ultimate' } } };
    const wire = translateAgentIdentityCommand('start_dialog_turn', input, 'legacy');
    expect(wire.request.agentType).toBe('agentic');
    expect(wire.request.modeId).toBe('Ultra');
    expect(wire.request.arguments).toEqual(input.request.arguments);
    expect(wire.request.userMessageMetadata).toEqual(input.request.userMessageMetadata);
    expect(translateAgentIdentityCommand('start_dialog_turn', wire, 'canonical')).toEqual(input);
    const external = { source: 'external', id: 'agentic', toolCount: 1 };
    expect(translateAgentIdentityCommand('get_available_modes', external, 'canonical')).toEqual(external);
  });
});
