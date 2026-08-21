// R-WF-15: legion -> workflow wording. These assertions lock the user-facing
// i18n strings (zh-CN / zh-TW / en-US) free of the "legion"/"军团" wording so
// the frontend no longer exposes the old concept. Backend structure (LegionPreset)
// is intentionally untouched and verified separately via git diff.
import { describe, expect, it } from 'vitest';
import zhAgents from '@/locales/zh-CN/scenes/agents.json';
import zhBasics from '@/locales/zh-CN/settings/basics.json';
import zhTwAgents from '@/locales/zh-TW/scenes/agents.json';
import zhTwBasics from '@/locales/zh-TW/settings/basics.json';
import enAgents from '@/locales/en-US/scenes/agents.json';
import enBasics from '@/locales/en-US/settings/basics.json';
import { getAgentBadge, getAgentDescription } from '@/app/scenes/agents/utils';
import type { AgentWithCapabilities } from '@/app/scenes/agents/agentsStore';

function collectStrings(value: unknown, out: string[]): void {
  if (typeof value === 'string') {
    out.push(value);
  } else if (Array.isArray(value)) {
    for (const item of value) collectStrings(item, out);
  } else if (value !== null && typeof value === 'object') {
    for (const child of Object.values(value)) collectStrings(child, out);
  }
}

function stringsOf(...sources: unknown[]): string[] {
  const out: string[] = [];
  for (const source of sources) collectStrings(source, out);
  return out;
}

// Minimal TFunction stub (only reads from the zh-CN agents locale).
function tZh(key: string, options?: { defaultValue?: string }): string {
  const walk = (obj: unknown, path: string[]): unknown =>
    path.length === 0 ? obj : walk((obj as Record<string, unknown>)?.[path[0]], path.slice(1));
  const value = walk(zhAgents, key.split('.'));
  if (typeof value === 'string') return value;
  return options?.defaultValue ?? key;
}
const tZhFn = tZh as Parameters<typeof getAgentBadge>[0];

const WORKFLOW_MODE_AGENT: Pick<AgentWithCapabilities, 'id' | 'name' | 'description'> = {
  id: 'Legion',
  name: 'Workflow',
  description: 'Multi-agent workflow commander: orchestrate agent sessions through a fractal deployment topology',
};

describe('R-WF-15 legion -> workflow wording (i18n zero-residual)', () => {
  it('zh-CN: no "军团" remains in agents + settings locales', () => {
    const found = stringsOf(zhAgents, zhBasics).filter((s) => s.includes('军团'));
    expect(found).toEqual([]);
  });

  it('zh-TW: no "軍團" remains in agents + settings locales', () => {
    const found = stringsOf(zhTwAgents, zhTwBasics).filter((s) => s.includes('軍團'));
    expect(found).toEqual([]);
  });

  it('en-US: no "legion" (case-insensitive) remains in agents + settings locales', () => {
    const found = stringsOf(enAgents, enBasics).filter((s) => /legion/i.test(s));
    expect(found).toEqual([]);
  });

  // Data-layer acceptance: the legacy "Legion" registry id must render as the
  // workflow badge and the workflow description override (never the old
  // "Legion"/"智能体" card naming).
  it('data layer: Legion mode renders workflow badge + workflow description (zh-CN)', () => {
    const badge = getAgentBadge(tZhFn, 'mode', 'builtin', 'Legion');
    expect(badge.label).toBe('工作流');
    const description = getAgentDescription(tZhFn, WORKFLOW_MODE_AGENT);
    expect(description).toContain('工作流');
    expect(description).not.toContain('Legion');
  });

  it('data layer: ACP description is differentiated (no generic "ACP agent")', () => {
    const description = getAgentDescription(tZhFn, {
      id: 'acp__opencode',
      name: 'OpenCode',
      description: 'External ACP coding agent: run delegated implementation and analysis through the configured ACP client',
    });
    expect(description).not.toBe('ACP agent');
    expect(description).toContain('ACP');
  });
});
