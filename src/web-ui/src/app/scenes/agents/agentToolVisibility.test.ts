import { describe, expect, it } from 'vitest';
import { isAgentProfileConfigurableToolName } from './agentToolVisibility';

describe('isAgentProfileConfigurableToolName', () => {
  it('hides MCP tools from per-agent profile configuration', () => {
    expect(isAgentProfileConfigurableToolName('mcp__github__list_issues')).toBe(false);
  });

  it('keeps regular product tools configurable', () => {
    expect(isAgentProfileConfigurableToolName('Read')).toBe(true);
  });

  it('continues to hide internal gateway tools', () => {
    expect(isAgentProfileConfigurableToolName('GetToolSpec')).toBe(false);
  });
});
