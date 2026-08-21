// @vitest-environment jsdom

import { describe, expect, it } from 'vitest';

import { isSessionRowPointerTarget } from './sessionOpenPointer';

describe('isSessionRowPointerTarget', () => {
  it('accepts pointer targets rendered inside the session row', () => {
    const row = document.createElement('div');
    const label = document.createElement('span');
    row.append(label);

    expect(isSessionRowPointerTarget(row, label)).toBe(true);
  });

  it('rejects pointer targets rendered through a portal outside the session row', () => {
    const row = document.createElement('div');
    const portalMenu = document.createElement('div');
    const deleteButton = document.createElement('button');
    portalMenu.append(deleteButton);
    document.body.append(row, portalMenu);

    expect(isSessionRowPointerTarget(row, deleteButton)).toBe(false);

    row.remove();
    portalMenu.remove();
  });
});
