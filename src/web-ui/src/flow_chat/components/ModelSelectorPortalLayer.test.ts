import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';

function readSource(relativePath: string): string {
  return readFileSync(
    fileURLToPath(new URL(relativePath, import.meta.url)),
    'utf8',
  ).replace(/\r\n/g, '\n');
}

describe('ModelSelector portal layer', () => {
  it('keeps the body-portaled menu above overlay-hosted chat surfaces', () => {
    const component = readSource('./ModelSelector.tsx');
    const stylesheet = readSource('./ModelSelector.scss');
    const dropdownBlock = stylesheet.match(
      /&__dropdown\s*\{(?<body>[\s\S]*?)\n\s*\}/,
    )?.groups?.body;

    expect(component).toContain('createPortal(');
    expect(component).toContain('document.body');
    expect(dropdownBlock).toContain('z-index: $z-popover;');
    expect(dropdownBlock).not.toContain('z-index: $z-dropdown;');
  });
});
