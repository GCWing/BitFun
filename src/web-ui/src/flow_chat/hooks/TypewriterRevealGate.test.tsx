// @vitest-environment jsdom
/**
 * Regression tests for TypewriterRevealGate.
 *
 * The reporter effect must depend on the stable `report` function only.
 * Depending on the whole gate object (new identity per report) used to cause
 * an infinite report → re-render → cleanup → report loop that froze the app
 * whenever a streaming typewriter was revealing.
 */
import React, { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, describe, expect, it } from 'vitest';
import {
  TypewriterRevealGateProvider,
} from './TypewriterRevealGate';
import {
  useCreateTypewriterRevealGate,
  useReportTypewriterReveal,
  useTypewriterRevealGate,
} from './typewriterRevealGateContext';

(globalThis as any).IS_REACT_ACT_ENVIRONMENT = true;

let container: HTMLDivElement;
let root: Root | null = null;

afterEach(() => {
  if (root) {
    act(() => root!.unmount());
    root = null;
  }
  container?.remove();
});

function Consumer({ id, revealing }: { id: string; revealing: boolean }) {
  useReportTypewriterReveal(id, revealing);
  return null;
}

function Observer({ onState }: { onState: (v: boolean) => void }) {
  const gate = useTypewriterRevealGate();
  onState(Boolean(gate?.isAnyRevealing));
  return null;
}

function Probe({
  consumers,
  onState,
}: {
  consumers: Array<{ id: string; revealing: boolean }>;
  onState: (v: boolean) => void;
}) {
  const gate = useCreateTypewriterRevealGate();
  return (
    <TypewriterRevealGateProvider value={gate}>
      {consumers.map((c) => (
        <Consumer key={c.id} id={c.id} revealing={c.revealing} />
      ))}
      <Observer onState={onState} />
    </TypewriterRevealGateProvider>
  );
}

function mount(consumers: Array<{ id: string; revealing: boolean }>, states: boolean[]) {
  container = document.createElement('div');
  document.body.appendChild(container);
  root = createRoot(container);
  act(() => {
    root!.render(<Probe consumers={consumers} onState={(v) => states.push(v)} />);
  });
}

describe('TypewriterRevealGate', () => {
  it('settles when a consumer reports revealing=true', () => {
    const states: boolean[] = [];
    mount([{ id: 'text-1', revealing: true }], states);

    // Should settle quickly, not re-render forever.
    expect(states.length).toBeLessThan(10);
    expect(states[states.length - 1]).toBe(true);
  });

  it('clears the gate when the last consumer stops revealing', () => {
    const states: boolean[] = [];
    mount([{ id: 'text-1', revealing: true }], states);
    expect(states[states.length - 1]).toBe(true);

    act(() => {
      root!.render(
        <Probe consumers={[{ id: 'text-1', revealing: false }]} onState={(v) => states.push(v)} />
      );
    });

    expect(states.length).toBeLessThan(20);
    expect(states[states.length - 1]).toBe(false);
  });

  it('stays gated while any consumer is still revealing', () => {
    const states: boolean[] = [];
    mount(
      [
        { id: 'text-1', revealing: true },
        { id: 'thinking-1', revealing: true },
      ],
      states
    );
    expect(states[states.length - 1]).toBe(true);

    act(() => {
      root!.render(
        <Probe
          consumers={[
            { id: 'text-1', revealing: false },
            { id: 'thinking-1', revealing: true },
          ]}
          onState={(v) => states.push(v)}
        />
      );
    });

    expect(states[states.length - 1]).toBe(true);
  });
});
