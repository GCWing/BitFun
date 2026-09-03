import { describe, expect, it, vi } from 'vitest';
import { createLocalFileDropController } from './useLocalFileDrop';

const target = {
  getBoundingClientRect: () => ({ left: 10, top: 10, right: 50, bottom: 50 }),
} as HTMLElement;

function setup(options: { enabled?: boolean; scaleFactor?: number } = {}) {
  const onDropPaths = vi.fn();
  const onDragOver = vi.fn();
  let now = 1000;
  const controller = createLocalFileDropController({
    getTarget: () => target,
    getScaleFactor: async () => options.scaleFactor ?? 1,
    isEnabled: () => options.enabled ?? true,
    onDropPaths,
    onDragOver,
    now: () => now,
  });
  return { controller, onDropPaths, onDragOver, advance: (ms: number) => { now += ms; } };
}

describe('local file drop controller', () => {
  it('uses enter paths when drop paths are empty', async () => {
    const { controller, onDropPaths } = setup();
    await controller.handle({ type: 'enter', paths: ['/tmp/a'], position: { x: 20, y: 20 } });
    await controller.handle({ type: 'drop', paths: [], position: { x: 20, y: 20 } });
    expect(onDropPaths).toHaveBeenCalledWith(['/tmp/a']);
  });

  it('uses the scale factor when hit testing', async () => {
    const { controller, onDropPaths } = setup({ scaleFactor: 2 });
    await controller.handle({ type: 'drop', paths: ['/tmp/a'], position: { x: 40, y: 40 } });
    expect(onDropPaths).toHaveBeenCalledOnce();
  });

  it('clears hover state on leave', async () => {
    const { controller, onDragOver } = setup();
    await controller.handle({ type: 'over', position: { x: 20, y: 20 } });
    await controller.handle({ type: 'leave' });
    expect(onDragOver).toHaveBeenLastCalledWith(false);
  });

  it('deduplicates repeated drops for 500ms', async () => {
    const { controller, onDropPaths, advance } = setup();
    const payload = { type: 'drop' as const, paths: ['/tmp/a'], position: { x: 20, y: 20 } };
    await controller.handle(payload);
    advance(400);
    await controller.handle(payload);
    expect(onDropPaths).toHaveBeenCalledOnce();
    advance(101);
    await controller.handle(payload);
    expect(onDropPaths).toHaveBeenCalledTimes(2);
  });

  it('ignores misses and disabled drops', async () => {
    const miss = setup();
    await miss.controller.handle({ type: 'over', position: { x: 20, y: 20 } });
    await miss.controller.handle({ type: 'drop', paths: ['/tmp/a'], position: { x: 100, y: 100 } });
    expect(miss.onDropPaths).not.toHaveBeenCalled();

    const disabled = setup({ enabled: false });
    await disabled.controller.handle({ type: 'drop', paths: ['/tmp/a'], position: { x: 20, y: 20 } });
    expect(disabled.onDropPaths).not.toHaveBeenCalled();
  });

  it('stops handling after dispose', async () => {
    const { controller, onDropPaths, onDragOver } = setup();
    controller.dispose();
    await controller.handle({ type: 'drop', paths: ['/tmp/a'], position: { x: 20, y: 20 } });
    expect(onDropPaths).not.toHaveBeenCalled();
    expect(onDragOver).toHaveBeenCalledWith(false);
  });
});
