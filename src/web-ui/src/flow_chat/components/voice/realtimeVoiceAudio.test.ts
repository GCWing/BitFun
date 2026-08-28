import { afterEach, describe, expect, it, vi } from 'vitest';
import { decodePcm16Base64, RealtimePcmPlayer } from './realtimeVoiceAudio';

class FakeAudioBuffer {
  readonly duration: number;
  samples: Float32Array | null = null;

  constructor(length: number, sampleRate: number) {
    this.duration = length / sampleRate;
  }

  copyToChannel(samples: Float32Array): void {
    this.samples = samples;
  }
}

class FakeAudioBufferSource {
  buffer: AudioBuffer | null = null;
  startedAt: number | null = null;
  stopped = false;

  connect(): void {}

  disconnect(): void {}

  addEventListener(): void {}

  start(when = 0): void {
    this.startedAt = when;
  }

  stop(): void {
    this.stopped = true;
  }
}

class FakeAudioContext {
  static latest: FakeAudioContext | null = null;

  readonly sampleRate: number;
  readonly currentTime = 1;
  readonly destination = {} as AudioDestinationNode;
  readonly sources: FakeAudioBufferSource[] = [];
  state: AudioContextState = 'suspended';
  resumeCalls = 0;

  constructor(options?: AudioContextOptions) {
    this.sampleRate = options?.sampleRate ?? 48_000;
    FakeAudioContext.latest = this;
  }

  createBuffer(_channels: number, length: number, sampleRate: number): AudioBuffer {
    return new FakeAudioBuffer(length, sampleRate) as unknown as AudioBuffer;
  }

  createBufferSource(): AudioBufferSourceNode {
    const source = new FakeAudioBufferSource();
    this.sources.push(source);
    return source as unknown as AudioBufferSourceNode;
  }

  async resume(): Promise<void> {
    this.resumeCalls += 1;
    this.state = 'running';
  }

  async close(): Promise<void> {
    this.state = 'closed';
  }
}

afterEach(() => {
  FakeAudioContext.latest = null;
  vi.unstubAllGlobals();
});

describe('realtime PCM playback', () => {
  it('decodes signed little-endian PCM16 samples', () => {
    vi.stubGlobal('window', { atob: globalThis.atob });

    const samples = decodePcm16Base64('AIAAAP9///8=');

    expect(Array.from(samples)).toEqual([
      -1,
      0,
      0x7fff / 0x8000,
      -1 / 0x8000,
    ]);
  });

  it('unlocks output during creation and schedules streamed audio', async () => {
    vi.stubGlobal('window', {
      atob: globalThis.atob,
      AudioContext: FakeAudioContext,
      webkitAudioContext: undefined,
    });

    const player = RealtimePcmPlayer.create(24_000);
    await player.enqueue('AAAAAA==');

    const context = FakeAudioContext.latest;
    expect(context).not.toBeNull();
    expect(context?.sampleRate).toBe(24_000);
    expect(player.getAudioContext()).toBe(context);
    expect(context?.state).toBe('running');
    expect(context?.resumeCalls).toBe(1);
    expect(context?.sources).toHaveLength(2);
    expect(context?.sources[1]?.startedAt).toBeGreaterThan(context?.currentTime ?? 0);

    await player.close();
    expect(context?.state).toBe('closed');
  });
});
