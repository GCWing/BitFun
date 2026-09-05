// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from 'vitest';
import { activateCreationRuntime } from './creationRuntime';

function loadedStyle() {
  document.querySelector('link[data-openbitfun-creation]')!.dispatchEvent(new Event('load'));
}

afterEach(() => { document.head.innerHTML = ''; document.body.innerHTML = ''; });

describe('packaged UI customization lifecycle', () => {
  it('waits for styles and activation before success, keeping overrides after lazy styles', async () => {
    const controller = new AbortController();
    const disposeApi = vi.fn();
    const deactivate = vi.fn();
    let finish!: (cleanup: () => void) => void;
    const activate = vi.fn(() => new Promise<() => void>(resolve => { finish = resolve; }));
    const loadModule = vi.fn(async () => ({ default: activate }));
    let ready = false;
    const pending = activateCreationRuntime({ api: {}, signal: controller.signal, disposeApi, loadModule })
      .then(dispose => { ready = true; return dispose; });
    expect(loadModule).not.toHaveBeenCalled();
    loadedStyle();
    await vi.waitFor(() => expect(activate).toHaveBeenCalledOnce());
    expect(ready).toBe(false);
    finish(deactivate);
    const dispose = await pending;
    const lazyStyle = document.createElement('style');
    document.head.append(lazyStyle);
    await Promise.resolve();
    expect(document.head.lastElementChild?.tagName).toBe('LINK');
    dispose(); dispose(); controller.abort();
    expect(deactivate).toHaveBeenCalledOnce();
    expect(disposeApi).toHaveBeenCalledOnce();
    expect(document.querySelector('link[data-openbitfun-creation]')).toBeNull();
  });

  it('cleans up and rejects a broken module instead of reporting a ready candidate', async () => {
    const disposeApi = vi.fn();
    const pending = activateCreationRuntime({ api: {}, signal: new AbortController().signal,
      disposeApi, loadModule: async () => { throw new SyntaxError('Invalid user module'); } });
    const rejected = expect(pending).rejects.toThrow('Invalid user module');
    loadedStyle();
    await rejected;
    expect(disposeApi).toHaveBeenCalledOnce();
    expect(document.querySelector('link')).toBeNull();
  });

  it('disposes late async activation after a surface switch', async () => {
    const controller = new AbortController();
    const disposeApi = vi.fn(); const deactivate = vi.fn();
    let finish!: (cleanup: () => void) => void;
    const pending = activateCreationRuntime({ api: {}, signal: controller.signal, disposeApi,
      loadModule: async () => ({ default: () => new Promise<() => void>(resolve => { finish = resolve; }) }) });
    const rejected = expect(pending).rejects.toMatchObject({ name: 'AbortError' });
    loadedStyle();
    await vi.waitFor(() => expect(finish).toBeTypeOf('function'));
    controller.abort(); finish(deactivate);
    await rejected;
    expect(deactivate).toHaveBeenCalledOnce();
    expect(disposeApi).toHaveBeenCalledOnce();
  });

  it('rejects missing CSS and accepts legacy side-effect-only modules', async () => {
    const missing = activateCreationRuntime({ api: {}, signal: new AbortController().signal, disposeApi: vi.fn() });
    const rejected = expect(missing).rejects.toThrow('stylesheet');
    document.querySelector('link')!.dispatchEvent(new Event('error'));
    await rejected;
    const legacy = activateCreationRuntime({ api: {}, signal: new AbortController().signal,
      disposeApi: vi.fn(), loadModule: async () => ({}) });
    loadedStyle(); (await legacy)();
  });
});
