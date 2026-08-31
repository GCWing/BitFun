import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';

function readStylesheet(): string {
  return readFileSync(
    fileURLToPath(new URL('./AcpAgentsConfig.scss', import.meta.url)),
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
      '@media (max-width: 860px)',
    );

    expect(remoteList).toContain('border-radius: inherit;');
    expect(remoteList).not.toContain('gap:');
    expect(hiddenRemoteList).not.toContain('border: 1px solid');
    expect(remoteServer).not.toContain('border: 1px solid');
    expect(remoteServer).not.toContain('border-radius:');
    expect(remoteServer).toContain(
      'border-top: 1px solid var(--bf-component-config-page-divider);',
    );
    expect(remoteAgentList).not.toContain('gap:');
    expect(remoteAgentList).not.toContain('padding:');
  });
});
