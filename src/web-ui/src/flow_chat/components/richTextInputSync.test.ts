import { describe, expect, it } from 'vitest';
import { getRichTextExternalSyncAction } from './richTextInputSync';

describe('getRichTextExternalSyncAction', () => {
  it('does nothing when parent value already matches DOM content', () => {
    expect(getRichTextExternalSyncAction('hello', 'hello')).toBe('noop');
  });

  it('clears the DOM when parent value becomes empty', () => {
    expect(getRichTextExternalSyncAction('', 'hello')).toBe('clear');
  });

  it('replaces the DOM when parent value diverges from current content', () => {
    expect(getRichTextExternalSyncAction('server rewrite', 'hello')).toBe('replace');
  });

  it('does nothing when both parent value and DOM are empty', () => {
    expect(getRichTextExternalSyncAction('', '')).toBe('noop');
  });
});
