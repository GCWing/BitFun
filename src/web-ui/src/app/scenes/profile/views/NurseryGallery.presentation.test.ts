import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';

function readSibling(filename: string): string {
  return readFileSync(
    fileURLToPath(new URL(filename, import.meta.url)),
    'utf8',
  ).replace(/\r\n/g, '\n');
}

describe('Nursery gallery panda presentation', () => {
  it('layers a dedicated wink frame over the stable panda avatar', () => {
    const source = readSibling('./NurseryGallery.tsx');

    expect(source).toContain('src="/panda_1.png"');
    expect(source).toContain('src="/panda_wink.png"');
    expect(source).toContain('nursery-defaults__avatar-image--wink');
  });

  it('keeps the wink local to the panda and disables it for reduced motion', () => {
    const stylesheet = readSibling('./NurseryView.scss');
    const reducedMotionStart = stylesheet.indexOf('@media (prefers-reduced-motion: reduce)');
    const reducedMotionEnd = stylesheet.indexOf('// ── Responsive', reducedMotionStart);
    const reducedMotionSection = stylesheet.slice(reducedMotionStart, reducedMotionEnd);

    expect(stylesheet).toMatch(
      /\.nursery-defaults__avatar:hover \.nursery-defaults__avatar-art[\s\S]*nursery-panda-wink-tilt/,
    );
    expect(stylesheet).toContain('@keyframes nursery-panda-wink-frame');
    expect(stylesheet).toMatch(/&--wink \{\s+opacity: 0;/);
    expect(reducedMotionSection).toContain('.nursery-defaults__avatar-art');
    expect(reducedMotionSection).toContain('.nursery-defaults__avatar-image--wink');
    expect(reducedMotionSection).toContain('animation: none;');
  });
});
