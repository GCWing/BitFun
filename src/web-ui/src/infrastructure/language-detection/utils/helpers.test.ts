import { describe, expect, it } from 'vitest';
import { getEditorType, getFileIconType } from './helpers';

describe('language detection editor routing', () => {
  it('routes SVG files to the image viewer', () => {
    expect(getFileIconType('icon.svg')).toBe('image');
    expect(getEditorType('icon.svg')).toBe('image-viewer');
  });

  it('keeps executable and archive files in the text editor route', () => {
    expect(getFileIconType('app.exe')).toBe('binary');
    expect(getFileIconType('bundle.zip')).toBe('archive');
    expect(getEditorType('app.exe')).toBe('code-editor');
    expect(getEditorType('bundle.zip')).toBe('code-editor');
  });
});
