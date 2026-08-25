import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';

const stylesheet = readFileSync(
  fileURLToPath(new URL('./WorkspaceBody.scss', import.meta.url)),
  'utf8',
).replace(/\r\n/g, '\n');

describe('WorkspaceBody presentation contract', () => {
  it('shares the split-view content-panel curvature with its resize feedback edge', () => {
    expect(stylesheet).toContain(
      '--_scene-surface-radius: var(--bf-layout-split-view-content-panel-radius);',
    );
    expect(stylesheet.match(
      /border-radius: var\(--_scene-surface-radius\) 0 0 var\(--_scene-surface-radius\);/g,
    )).toHaveLength(2);
    expect(stylesheet).toContain('width: var(--_scene-surface-radius);');
    expect(stylesheet).not.toContain(
      'border-radius: $size-radius-xl 0 0 $size-radius-xl;',
    );
  });
});
