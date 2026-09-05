// @vitest-environment jsdom
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { JSDOM } from 'jsdom';
import { createCreationCapabilities } from './creationCapabilities';

const localStorage = new JSDOM('', { url: 'https://creation.test' }).window.localStorage;
const runtime = () => createCreationCapabilities({ assertActive: () => {}, storage: localStorage });
beforeEach(() => localStorage.clear());

describe('Creation capabilities', () => {
  it('discovers schemas and composes command, state and events, surviving runtime replacement', async () => {
    const first = runtime();
    const subscriber = vi.fn();
    first.events.on('state.changed', subscriber);
    first.commands.register({ id: 'counter.increment', description: 'Add to the counter', parameters: { amount: { type: 'integer', required: true } } }, async args => {
      return first.state.set('counter.value', Number(first.state.get('counter.value', 0)) + Number(args.amount));
    });
    expect(first.inspect().commands[0].inputSchema.required).toEqual(['amount']);
    expect(await first.commands.invoke('counter.increment', { amount: 3 })).toBe(3);
    expect(subscriber).toHaveBeenCalledWith({ key: 'counter.value', value: 3 });
    first.dispose();
    expect(() => first.inspect()).toThrow('deactivated');
    const next = runtime();
    expect(next.state.get('counter.value')).toBe(3);
    expect(next.inspect().commands).toEqual([]);
    expect(next.inspect().events).toEqual([]);
  });

  it('rejects invalid input before executing and makes errors inspectable', async () => {
    const current = runtime();
    const handler = vi.fn();
    current.commands.register({ id: 'test.run', description: 'Run', parameters: { amount: { type: 'integer', required: true } } }, handler);
    await expect(current.commands.invoke('test.run', { amount: 1.2 })).rejects.toThrow('integer');
    await expect(current.commands.invoke('test.run')).rejects.toThrow('Missing');
    await expect(current.commands.invoke('test.run', { amount: 1, unrelated: true })).rejects.toThrow('Unknown');
    expect(handler).not.toHaveBeenCalled();
    expect(current.inspect().diagnostics).toHaveLength(3);
    current.commands.register({ id: 'test.fail', description: 'Fail' }, () => { throw new Error('Useful runtime error'); });
    await expect(current.commands.invoke('test.fail')).rejects.toThrow('Useful runtime error');
    expect(current.inspect().diagnostics.slice(-1)[0]?.message).toBe('Useful runtime error');
  });

  it('keeps unreadable state and rejects implicit repair while allowing explicit deletion', async () => {
    const key = 'openbitfun.creation.state.v1:counter.value';
    const current = runtime();
    localStorage.setItem(key, '{broken');
    await expect(current.state.set('counter.value', 5)).rejects.toThrow('preserved');
    expect(localStorage.getItem(key)).toBe('{broken');
    localStorage.setItem(key, JSON.stringify({ schemaVersion: 2, value: 8 }));
    expect(() => current.state.get('counter.value')).toThrow('preserved');
    await current.state.delete('counter.value');
    await current.state.set('counter.value', { count: 1 });
    const copy = current.state.get('counter.value') as { count: number }; copy.count = 9;
    expect(current.state.get('counter.value')).toEqual({ count: 1 });
  });

  it('contains observer errors without misreporting persisted writes and ignores stale unregister handles', async () => {
    const current = runtime();
    current.events.on('state.changed', () => { throw new Error('Observer failed'); });
    expect(await current.state.set('counter.value', 7)).toBe(7);
    expect(current.inspect().diagnostics[0].message).toBe('Observer failed');
    const remove = current.commands.register({ id: 'counter.read', description: 'Read' }, () => 1);
    remove();
    current.commands.register({ id: 'counter.read', description: 'Read again' }, () => 2);
    remove();
    expect(await current.commands.invoke('counter.read')).toBe(2);
  });
});
