import { describe, expect, it } from 'vitest';
import { resolveTodoLineage, type TodoLineageLike } from './todoLineage';

function todo(id: string | null | undefined, dependencies?: string[]): TodoLineageLike {
  return id == null ? { dependencies } : { id, dependencies };
}

describe('resolveTodoLineage', () => {
  it('returns empty items for an empty input', () => {
    const result = resolveTodoLineage([]);
    expect(result.items).toEqual([]);
    expect(result.hasCycle).toBe(false);
  });

  it('keeps the original order with depth 0 when there are no dependencies', () => {
    const todos = [todo('a'), todo('b'), todo('c')];
    const result = resolveTodoLineage(todos);
    expect(result.hasCycle).toBe(false);
    expect(result.items.map((item) => item.todo.id)).toEqual(['a', 'b', 'c']);
    expect(result.items.map((item) => item.depth)).toEqual([0, 0, 0]);
  });

  it('orders a parent before its dependent with depth 1', () => {
    const todos = [todo('child', ['parent']), todo('parent')];
    const result = resolveTodoLineage(todos);
    expect(result.hasCycle).toBe(false);
    expect(result.items.map((item) => item.todo.id)).toEqual(['parent', 'child']);
    expect(result.items.map((item) => item.depth)).toEqual([0, 1]);
  });

  it('computes depths along a chain', () => {
    const todos = [todo('c', ['b']), todo('b', ['a']), todo('a')];
    const result = resolveTodoLineage(todos);
    expect(result.hasCycle).toBe(false);
    expect(result.items.map((item) => item.todo.id)).toEqual(['a', 'b', 'c']);
    expect(result.items.map((item) => item.depth)).toEqual([0, 1, 2]);
  });

  it('falls back to flat order with hasCycle=true on a cycle', () => {
    const todos = [todo('a', ['b']), todo('b', ['a'])];
    const result = resolveTodoLineage(todos);
    expect(result.hasCycle).toBe(true);
    expect(result.items.map((item) => item.todo.id)).toEqual(['a', 'b']);
    expect(result.items.map((item) => item.depth)).toEqual([0, 0]);
  });

  it('ignores a self-loop edge (no cycle)', () => {
    const todos = [todo('a', ['a'])];
    const result = resolveTodoLineage(todos);
    expect(result.hasCycle).toBe(false);
    expect(result.items.map((item) => item.todo.id)).toEqual(['a']);
    expect(result.items.map((item) => item.depth)).toEqual([0]);
  });

  it('ignores an unknown dependency reference (no cycle)', () => {
    const todos = [todo('a', ['missing'])];
    const result = resolveTodoLineage(todos);
    expect(result.hasCycle).toBe(false);
    expect(result.items.map((item) => item.todo.id)).toEqual(['a']);
    expect(result.items.map((item) => item.depth)).toEqual([0]);
  });

  it('uses the first dependency as the parent when a todo has multiple parents', () => {
    const todos = [todo('child', ['first', 'second']), todo('first'), todo('second')];
    const result = resolveTodoLineage(todos);
    expect(result.hasCycle).toBe(false);
    expect(result.items.map((item) => item.todo.id)).toEqual(['first', 'second', 'child']);
    expect(result.items.map((item) => item.depth)).toEqual([0, 0, 1]);
  });

  it('keeps id-less todos flat and appends them in original order', () => {
    const todos = [todo('child', ['parent']), todo(undefined), todo('parent')];
    const result = resolveTodoLineage(todos);
    expect(result.hasCycle).toBe(false);
    expect(result.items.map((item) => item.todo.id)).toEqual(['parent', 'child', undefined]);
    expect(result.items.map((item) => item.depth)).toEqual([0, 1, 0]);
  });

  it('keeps duplicate-id todos flat while the first occurrence participates in the graph', () => {
    const todos = [todo('dup', ['root']), todo('dup'), todo('root')];
    const result = resolveTodoLineage(todos);
    expect(result.hasCycle).toBe(false);
    expect(result.items.map((item) => item.todo.id)).toEqual(['root', 'dup', 'dup']);
    expect(result.items.map((item) => item.depth)).toEqual([0, 1, 0]);
  });
});
