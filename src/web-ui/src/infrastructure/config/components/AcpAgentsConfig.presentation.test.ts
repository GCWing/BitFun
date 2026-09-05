import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';

function readStylesheet(): string {
  return readFileSync(
    fileURLToPath(new URL('./AcpAgentsConfig.scss', import.meta.url)),
    'utf8',
  ).replace(/\r\n/g, '\n');
}

function readSource(relativePath: string): string {
  return readFileSync(
    fileURLToPath(new URL(relativePath, import.meta.url)),
    'utf8',
  ).replace(/\r\n/g, '\n');
}

function readBlock(stylesheet: string, start: string, end: string): string {
  const startIndex = stylesheet.indexOf(start);
  const endIndex = stylesheet.indexOf(end, startIndex);

  expect(startIndex).toBeGreaterThan(-1);
  expect(endIndex).toBeGreaterThan(startIndex);

  return stylesheet.slice(startIndex, endIndex);
}

describe('ACP Agent settings presentation', () => {
  it('delegates capability, agent status, and remote summary badges to the design system', () => {
    const source = readSource('./AcpAgentsConfig.tsx');
    const stylesheet = readStylesheet();

    expect(source).toContain('StatusPill');
    expect(source).toContain('type StatusPillTone');
    expect(source).toContain('function CapabilityStatusPill');
    expect(source).toContain('function AgentStatusPill');
    expect(source.match(/<CapabilityStatusPill/g)).toHaveLength(2);
    expect(source).toContain('openbitfun-acp-agents__registry-row--remote');
    expect(source).toContain('<Spinner size="xs" />');
    expect(source).not.toContain('LoaderCircle');
    expect(stylesheet).not.toContain('&__capability {');
    expect(stylesheet).not.toContain('&__status {');
    expect(stylesheet).not.toContain('&__summary-pill {');
    expect(stylesheet).not.toContain('border-radius: 999px;');
    expect(stylesheet).not.toContain('@keyframes openbitfun-acp-spin');
  });

  it('separates local, SSH, and advanced JSON views with scoped draft guards', () => {
    const source = readSource('./AcpAgentsConfig.tsx');
    const appearance = readSource('./AcpAgentsConfig.appearance.ts');

    expect(source).toContain('<TabGroup');
    expect(source).toContain("activeView === 'local'");
    expect(source).toContain("activeView === 'ssh'");
    expect(source).toContain("activeView === 'json'");
    expect(source).toContain("activeView !== 'ssh'");
    expect(source).toContain('jsonDirty');
    expect(source).toContain('useSettingsDraft({');
    expect(source).toContain("viewId: activeView === 'json' && jsonDirty ? 'json' : undefined");
    expect(source).toContain('enabled: settingsDraftEnabled');
    expect(source).toContain("if (!settingsDraftEnabled && activeView === 'json' && jsonDirty)");
    expect(source).toContain('discardJsonChanges');
    expect(source).toContain("open={!settingsDraftEnabled && pendingView !== null}");
    expect(source).toContain("window.addEventListener('beforeunload', handleBeforeUnload)");
    expect(appearance).toContain("values: ['local', 'ssh', 'json']");
  });

  it('keeps saved SSH servers inside the shared section surface', () => {
    const stylesheet = readStylesheet();
    const remoteList = readBlock(
      stylesheet,
      '&__remote-list {',
      '&__hidden-remote-list {',
    );
    const hiddenRemoteList = readBlock(
      stylesheet,
      '&__hidden-remote-list {',
      '&__hidden-remote-row {',
    );
    const remoteServer = readBlock(
      stylesheet,
      '&__remote-server {',
      '&__remote-head {',
    );
    const remoteAgentList = readBlock(
      stylesheet,
      '&__remote-agent-list {',
      '@container config-panel (max-width: 640px)',
    );

    expect(remoteList).toContain('border-radius: inherit;');
    expect(remoteList).not.toContain('gap:');
    expect(hiddenRemoteList).not.toContain('border: 1px solid');
    expect(remoteServer).not.toContain('border: 1px solid');
    expect(remoteServer).not.toContain('border-radius:');
    expect(remoteServer).toContain(
      'border-top: 1px solid var(--openbitfun-component-config-page-divider);',
    );
    expect(remoteAgentList).not.toContain('gap:');
    expect(remoteAgentList).not.toContain('padding:');
  });
});
