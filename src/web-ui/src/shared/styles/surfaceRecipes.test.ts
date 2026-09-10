import { fileURLToPath } from 'node:url';
import { compileString } from 'sass';
import { describe, expect, it } from 'vitest';

const compiled = compileString(
  `
    @use 'surface-recipes' as surfaces;

    .frosted { @include surfaces.floating; }
    .opaque { @include surfaces.floating($backdrop-blur: false); }
    .dialog { @include surfaces.dialog; }
  `,
  {
    loadPaths: [fileURLToPath(new URL('.', import.meta.url))],
    style: 'expanded',
  },
).css;

describe('surface recipes', () => {
  it('uses the complete blur filter token for frosted floating surfaces', () => {
    expect(compiled).toContain(
      '-webkit-backdrop-filter: var(--openbitfun-effect-blur-medium);',
    );
    expect(compiled).toContain(
      'backdrop-filter: var(--openbitfun-effect-blur-medium);',
    );
    expect(compiled).not.toContain(
      'blur(var(--openbitfun-effect-blur-medium))',
    );
  });

  it('keeps an opaque fallback and honors reduced-transparency preferences', () => {
    expect(compiled).toMatch(
      /\.frosted\s*\{[^}]*background:\s*var\(--openbitfun-color-surface-raised\);/,
    );
    expect(compiled).toMatch(
      /@supports[^{}]*backdrop-filter[^{}]*\{\s*\.frosted\s*\{[^}]*background:\s*color-mix\(/,
    );
    expect(compiled).toMatch(
      /@media \(prefers-reduced-transparency: reduce\)[^{]*\{\s*\.frosted\s*\{[^}]*background:\s*var\(--openbitfun-color-surface-raised\);[^}]*backdrop-filter:\s*none;/,
    );
  });

  it('leaves explicitly opaque floating surfaces and dialogs unblurred', () => {
    expect(compiled).toMatch(
      /\.opaque\s*\{[^}]*background:\s*var\(--openbitfun-color-surface-raised\);[^}]*\}/,
    );
    expect(compiled).toMatch(
      /\.dialog\s*\{[^}]*background:\s*var\(--openbitfun-color-surface-raised\);[^}]*\}/,
    );
    expect(compiled.match(/\.opaque\s*\{/g)).toHaveLength(1);
    expect(compiled.match(/\.dialog\s*\{/g)).toHaveLength(1);
  });
});
