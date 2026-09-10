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

    expect(source).toContain('{installedFiltered.map((skill) => (');
    expect(source).toMatch(/<ScrollArea\s+className="skills-main__grid"/);
    expect(source).not.toContain('INSTALLED_PAGE_SIZE');
    expect(source).not.toContain('skills-installed__pagination');
  });

  it('lets short lists end with their final row and constrains long lists to the scene', () => {
    const stylesheet = readSibling('./SkillsScene.scss');
    const listStylesheet = readSibling('./components/_SkillsList.scss');
    const shellStart = stylesheet.indexOf('.skills-main__list-shell {');
    const shellEnd = stylesheet.indexOf('.skills-main__list-header,', shellStart);
    const scrollStart = stylesheet.indexOf('.skills-main__grid,');
    const scrollEnd = stylesheet.indexOf('.skills-main__grid {', scrollStart);

    expect(stylesheet.slice(shellStart, shellEnd)).toContain('overflow: hidden;');
    expect(stylesheet.slice(shellStart, shellEnd)).toContain('padding: var(--openbitfun-space-2) var(--openbitfun-space-6) var(--openbitfun-space-6);');
    expect(stylesheet.slice(scrollStart, scrollEnd)).toContain('flex: 0 1 auto;');
    expect(stylesheet).toContain('@include skills-list.row;');
    expect(listStylesheet).toContain('min-block-size: 88px;');
  });

  it('uses the ecosystem navigation width and shared content typography', () => {
    const stylesheet = readSibling('./SkillsScene.scss');

    expect(stylesheet).toContain('$skills-sidebar-width: 264px;');
    expect(stylesheet).toContain('min-height: 32px;');
    expect(stylesheet).toContain('font-size: var(--openbitfun-type-heading-dialog-font-size);');
  });

  it('lets the skills page inherit the surrounding scene surface', () => {
    const stylesheet = readSibling('./SkillsScene.scss');
    const listStylesheet = readSibling('./components/_SkillsList.scss');
    const listSurfaceStart = stylesheet.indexOf('.skills-main__table {');
    const listSurfaceEnd = stylesheet.indexOf('\n}', listSurfaceStart);
    const surfaceStart = listStylesheet.indexOf('@mixin surface {');
    const surfaceEnd = listStylesheet.indexOf('\n}', surfaceStart);
    const headerStart = stylesheet.indexOf('.skills-content-header {');
    const headerEnd = stylesheet.indexOf('\n}', headerStart);

    expect(stylesheet).not.toContain('background: var(--openbitfun-color-surface-canvas);');
    expect(stylesheet.slice(listSurfaceStart, listSurfaceEnd)).toContain('@include skills-list.surface;');
    expect(listStylesheet.slice(surfaceStart, surfaceEnd)).toContain('background: transparent;');
    expect(stylesheet.slice(headerStart, headerEnd)).not.toContain('background:');
  });

  it('keeps the group scrollbar on the scene edge without moving its content', () => {
    const stylesheet = readSibling('./components/SkillGroupsView.scss');
    expect(stylesheet).toContain('--skill-groups-inline-inset: var(--openbitfun-space-6);');
    expect(stylesheet).toContain(
      'padding-inline: var(--skill-groups-inline-inset) calc(var(--skill-groups-inline-inset) + var(--openbitfun-space-1));',
    );
  });

  it('keeps row navigation and destructive actions as separate compact targets', () => {
    const source = readSibling('./SkillsScene.tsx');
    const stylesheet = readSibling('./SkillsScene.scss');

    expect(source).toContain('className="skills-card__actions"');
    expect(source).toContain('data-openbitfun-part="installedCardDetails"');
    expect(source).toContain('data-openbitfun-part="installedCardDelete"');
    expect(stylesheet).toContain('minmax(120px, 0.85fr) 64px;');
    expect(stylesheet).toContain('.skills-card__actions {');
  });
});
