import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';

function readSibling(filename: string): string {
  return readFileSync(
    fileURLToPath(new URL(filename, import.meta.url)),
    'utf8',
  ).replace(/\r\n/g, '\n');
}

describe('Skills scene presentation', () => {
  it('keeps the installed collection in one continuous scroll region', () => {
    const source = readSibling('./SkillsScene.tsx');

    expect(source).toContain('{installedFiltered.map((skill, index) => (');
    expect(source).not.toContain('INSTALLED_PAGE_SIZE');
    expect(source).not.toContain('skills-installed__pagination');
  });

  it('lets short lists end with their final row and constrains long lists to the scene', () => {
    const stylesheet = readSibling('./SkillsScene.scss');
    const shellStart = stylesheet.indexOf('.skills-main__list-shell {');
    const shellEnd = stylesheet.indexOf('.skills-main__list-header,', shellStart);
    const scrollStart = stylesheet.indexOf('.skills-main__grid,');
    const scrollEnd = stylesheet.indexOf('.skills-main__grid {', scrollStart);

    expect(stylesheet.slice(shellStart, shellEnd)).toContain('overflow: hidden;');
    expect(stylesheet.slice(scrollStart, scrollEnd)).toContain('flex: 0 1 auto;');
    expect(stylesheet).toContain('min-height: 64px;');
  });

  it('uses compact dimensions that fit the default desktop scene', () => {
    const stylesheet = readSibling('./SkillsScene.scss');

    expect(stylesheet).toContain('$skills-sidebar-width: 184px;');
    expect(stylesheet).toContain('min-height: 32px;');
    expect(stylesheet).toContain('font-size: var(--bf-font-size-2xl);');
  });

  it('keeps row navigation and destructive actions as separate compact targets', () => {
    const source = readSibling('./SkillsScene.tsx');
    const stylesheet = readSibling('./SkillsScene.scss');

    expect(source).toContain('className="skills-card__actions"');
    expect(source).toContain('data-bf-part="installedCardDetails"');
    expect(source).toContain('data-bf-part="installedCardDelete"');
    expect(stylesheet).toContain('64px 64px;');
    expect(stylesheet).toContain('.skills-card__actions {');
  });
});
