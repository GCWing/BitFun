// @vitest-environment jsdom

import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { compile } from 'sass';
import { describe, expect, it } from 'vitest';

const navSource = readFileSync(resolve(__dirname, 'FileViewerNav.tsx'), 'utf8').replace(
  /\r\n/g,
  '\n',
);

function compileRules(stylesheetPath: string): CSSStyleRule[] {
  const styleElement = document.createElement('style');
  styleElement.textContent = compile(stylesheetPath).css;
  document.head.appendChild(styleElement);
  const rules = Array.from(styleElement.sheet!.cssRules).filter(
    (rule): rule is CSSStyleRule => rule instanceof CSSStyleRule,
  );
  styleElement.remove();
  return rules;
}

const navRules = compileRules(resolve(__dirname, 'FileViewerNav.scss'));
const filesPanelRules = compileRules(
  resolve(__dirname, '../../components/panels/FilesPanel.scss'),
);

function declarations(rules: CSSStyleRule[], selector: string): CSSStyleDeclaration {
  const matches = rules.filter((entry) => entry.selectorText === selector);
  expect(matches.length, `Missing style rule: ${selector}`).toBeGreaterThan(0);
  const merged = document.createElement('div').style;
  for (const rule of matches) {
    for (let index = 0; index < rule.style.length; index += 1) {
      const property = rule.style.item(index);
      merged.setProperty(property, rule.style.getPropertyValue(property));
    }
  }
  return merged;
}

describe('FileViewerNav surface ownership', () => {
  it('hosts the files panel inside the shared NavigationPanel shell', () => {
    expect(navSource).toContain('<NavigationPanel');
    expect(navSource).toContain('className="openbitfun-file-viewer-nav"');
    expect(navSource).toMatch(
      /<NavigationPanelContent[^>]*>[\s\S]*?<FilesPanel[\s\S]*?<\/NavigationPanelContent>/,
    );
    expect(declarations(navRules, '.openbitfun-file-viewer-nav').background).toBe('transparent');
  });

  it('lets tree and search containers inherit the navigation background', () => {
    for (const selector of [
      '.openbitfun-files-panel',
      '.openbitfun-files-panel__content',
      '.openbitfun-files-panel__explorer',
      '.openbitfun-files-panel__search-results.openbitfun-search-results',
      '.openbitfun-files-panel__search-results.openbitfun-search-results .openbitfun-search-results__header',
    ]) {
      expect(declarations(filesPanelRules, selector).background).toBe('transparent');
    }
  });
});
