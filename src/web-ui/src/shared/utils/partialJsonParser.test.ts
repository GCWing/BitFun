import { describe, expect, it } from 'vitest';
import {
  extractFilePathFromJsonBuffer,
  getFirstAvailableField,
  isFieldComplete,
  parsePartialJson,
  splitFilePathAndContent,
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
  it('splits the first line from Write file content', () => {
    expect(splitFilePathAndContent('+++ C:/workspace/app.ts\r\nconst value = 1;')).toEqual({
      filePath: 'C:/workspace/app.ts',
      content: 'const value = 1;',
    });
  });

  it('extracts the first-line path while combined Write content is streaming', () => {
    const buffer = '{"payload":"+++ src/app.ts\\nconst value = 1;';

    expect(extractFilePathFromJsonBuffer(buffer)).toBe('src/app.ts');
  });

  it('treats a combined Write value without a newline as a partial path', () => {
    const buffer = '{"payload":"+++ C:/workspace/src/gener';

    expect(extractFilePathFromJsonBuffer(buffer)).toBe('C:/workspace/src/gener');
    expect(splitFilePathAndContent('+++ C:/workspace/src/gener')).toEqual({
      filePath: 'C:/workspace/src/gener',
      content: '',
    });
  });
  it('does not treat malformed payload content as a file path', () => {
    const buffer = '{"payload":"def main():\\n  pass';

    expect(extractFilePathFromJsonBuffer(buffer)).toBe('');
    expect(splitFilePathAndContent('def main():\n  pass')).toBeNull();
  });

});
