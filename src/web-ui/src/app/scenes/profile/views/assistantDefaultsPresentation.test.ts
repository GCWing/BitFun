import { describe, expect, it } from 'vitest';
import {
  isMcpServerAvailable,
  matchesAssistantDefaultsFilter,
  summarizeAssistantDefaults,
  type AssistantDefaultsFilterableItem,
} from './assistantDefaultsPresentation';

const items: AssistantDefaultsFilterableItem[] = [
  {
    name: 'Web scraper',
    description: 'Read structured content from a page',
    source: 'BitFun official',
    originalId: 'bitfun.web.scraper',
    enabled: true,
    defaultEnabled: false,
    available: true,
  },
  {
    name: 'Project interpreter',
    description: 'Run code in the project environment',
    source: 'Project',
    originalId: 'project.interpreter',
    enabled: false,
    defaultEnabled: false,
    available: false,
  },
];

describe('assistant defaults presentation', () => {
  it('summarizes enabled, changed, and unavailable capabilities independently', () => {
    expect(summarizeAssistantDefaults(items)).toEqual({
      enabled: 1,
      changed: 1,
      unavailable: 1,
    });
  });

  it('matches names, descriptions, sources, and original ids', () => {
    expect(matchesAssistantDefaultsFilter(items[0], 'all', 'structured')).toBe(true);
    expect(matchesAssistantDefaultsFilter(items[0], 'all', 'bitfun.web')).toBe(true);
    expect(matchesAssistantDefaultsFilter(items[1], 'unavailable', 'project')).toBe(true);
    expect(matchesAssistantDefaultsFilter(items[1], 'enabled', 'project')).toBe(false);
  });

  it('treats only connected, healthy MCP servers as currently available', () => {
    expect(isMcpServerAvailable(undefined)).toBe(true);
    expect(isMcpServerAvailable({ enabled: true, status: 'Healthy' })).toBe(true);
    expect(isMcpServerAvailable({ enabled: true, status: 'Connected' })).toBe(true);
    expect(isMcpServerAvailable({ enabled: true, status: 'NeedsAuth' })).toBe(false);
    expect(isMcpServerAvailable({ enabled: false, status: 'Healthy' })).toBe(false);
  });
});
