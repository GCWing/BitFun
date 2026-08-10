import { describe, expect, it } from 'vitest';
import { safeExternalUrl } from './DetailPage';

describe('safeExternalUrl', () => {
  it('accepts credential-free HTTPS links', () => {
    expect(safeExternalUrl('https://example.com/license')).toBe('https://example.com/license');
  });

  it('rejects active, insecure, and credential-bearing links', () => {
    expect(safeExternalUrl('javascript:alert(1)')).toBeUndefined();
    expect(safeExternalUrl('http://example.com/license')).toBeUndefined();
    expect(safeExternalUrl('https://user:secret@example.com/license')).toBeUndefined();
  });
});
