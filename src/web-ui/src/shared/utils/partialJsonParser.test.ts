import { describe, expect, it } from 'vitest';
import {
  extractFilePathFromJsonBuffer,
  getFirstAvailableField,
  isFieldComplete,
  parsePartialJson,
} from './partialJsonParser';

describe('partialJsonParser', () => {
  it('treats non-object partial fragments as empty params', () => {
    const partialString = '"from';

    expect(parsePartialJson(partialString)).toEqual({});
    expect(isFieldComplete(partialString, 'content')).toBe(false);
    expect(getFirstAvailableField(partialString, ['content', 'contents'])).toBeUndefined();
  });

  it('treats valid non-object JSON values as empty params', () => {
    expect(parsePartialJson('["content"]')).toEqual({});
    expect(parsePartialJson('true')).toEqual({});
    expect(parsePartialJson('42')).toEqual({});
  });

  it('treats non-string parser input as empty params', () => {
    expect(parsePartialJson({ content: 'not a JSON string' } as any)).toEqual({});
  });

  it('extracts file_path before content while content is still streaming', () => {
    const buffer = '{"file_path":"src/app.ts","content":"const value = 1;';

    expect(parsePartialJson(buffer).file_path).toBe('src/app.ts');
    expect(extractFilePathFromJsonBuffer(buffer)).toBe('src/app.ts');
  });

  it('does not treat file_path substrings inside a streaming content body as real paths', () => {
    const buffer = '{"content":"example \\"file_path\\": \\"fake.ts\\" text still open';

    expect(extractFilePathFromJsonBuffer(buffer)).toBe('');
  });

  it('extracts partial file_path values without a closing quote', () => {
    expect(extractFilePathFromJsonBuffer('{"file_path":"src/gener')).toBe('src/gener');
  });
});
