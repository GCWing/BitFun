import { describe, expect, it } from 'vitest';

import { ALL_NAMESPACES } from './namespaceRegistry';

describe('namespaceRegistry', () => {
  it('contains unique namespaces in stable sorted order', () => {
    const namespaces = [...ALL_NAMESPACES];

    expect(new Set(namespaces).size).toBe(namespaces.length);
    expect(namespaces).toEqual([...namespaces].sort());
  });

  it('includes the default common namespace', () => {
    expect(ALL_NAMESPACES).toContain('common');
  });
});
