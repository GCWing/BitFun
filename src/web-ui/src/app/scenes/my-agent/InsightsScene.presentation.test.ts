import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { describe, expect, it } from 'vitest';

describe('InsightsScene scroll layout', () => {
  it('keeps the list scroll viewport full-width and constrains only its content', () => {
    const source = readFileSync(resolve(__dirname, 'InsightsScene.tsx'), 'utf8');
    const styles = readFileSync(resolve(__dirname, 'InsightsScene.scss'), 'utf8');
    const listRootStart = styles.indexOf('.insights-scene:not(.insights-scene--report) {');
    const listRootEnd = styles.indexOf('\n}', listRootStart);
    const listRoot = styles.slice(listRootStart, listRootEnd);

    expect(source).toContain('className="insights-scene__history-inner"');
    expect(listRoot).toContain('width: 100%;');
    expect(listRoot).not.toContain('max-width:');
    expect(styles).toMatch(/\.insights-scene__history-inner\s*\{[\s\S]*?padding: \$ins-xs \$ins-lg \$ins-lg;/);
    expect(styles).toMatch(/\.insights-scene__history\s*\{\s*flex: 1;\s*min-height: 0;\s*\}/);
  });
});
