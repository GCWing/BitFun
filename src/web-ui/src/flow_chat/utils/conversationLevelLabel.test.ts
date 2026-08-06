import { describe, expect, it } from 'vitest';
import { conversationLevelLabel } from './conversationLevelLabel';

describe('conversationLevelLabel', () => {
  it('maps ancestor chain depth 0 to 主会话', () => {
    expect(conversationLevelLabel({ level: 0, isDescendant: false })).toBe('主会话');
  });

  it('maps ancestor chain depth 1 to 副官', () => {
    expect(conversationLevelLabel({ level: 1, isDescendant: false })).toBe('副官');
  });

  it('maps ancestor chain depth 2 to 上尉', () => {
    expect(conversationLevelLabel({ level: 2, isDescendant: false })).toBe('上尉');
  });

  it('maps ancestor chain depth 3 to 少尉', () => {
    expect(conversationLevelLabel({ level: 3, isDescendant: false })).toBe('少尉');
  });

  it('maps ancestor chain depth 4 and beyond to 士官', () => {
    expect(conversationLevelLabel({ level: 4, isDescendant: false })).toBe('士官');
    expect(conversationLevelLabel({ level: 10, isDescendant: false })).toBe('士官');
  });

  it('maps descendant entries to the same rank plus a peer sequence number', () => {
    expect(conversationLevelLabel({ level: 1, isDescendant: true })).toBe('副官 1');
    expect(conversationLevelLabel({ level: 2, isDescendant: true })).toBe('副官 2');
    expect(conversationLevelLabel({ level: 3, isDescendant: true })).toBe('副官 3');
  });

  it('keeps the raw label for an impossible negative ancestor level', () => {
    expect(conversationLevelLabel({ level: -1, isDescendant: false })).toBe('L-1');
  });
});
