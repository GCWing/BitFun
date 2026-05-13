import { describe, expect, it } from 'vitest';
import {
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
});
