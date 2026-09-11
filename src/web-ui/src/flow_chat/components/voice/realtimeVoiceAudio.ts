export function decodePcm16Base64(pcm16Base64: string): Float32Array {
  const binary = window.atob(pcm16Base64);
  const sampleCount = Math.floor(binary.length / 2);
  const samples = new Float32Array(sampleCount);
  for (let index = 0; index < sampleCount; index += 1) {
    const low = binary.charCodeAt(index * 2);
    const high = binary.charCodeAt(index * 2 + 1);
    const unsigned = low | (high << 8);
    const signed = unsigned >= 0x8000 ? unsigned - 0x10000 : unsigned;
    // The Volcengine duplex demo writes signed little-endian PCM16 directly
    // to a 24 kHz paInt16 stream. Dividing every sample by 32768 mirrors that
    // conversion exactly in Web Audio.
    samples[index] = signed / 0x8000;
  }
  return samples;
}

export class RealtimePcmPlayer {
  private readonly context: AudioContext;
  private sourceSampleRate: number;
  private nextStartTime = 0;
  private sources = new Set<AudioBufferSourceNode>();
  private resumeInFlight: Promise<void> | null = null;
  private readonly analyser: AnalyserNode;
  private readonly frequencyData: Uint8Array<ArrayBuffer>;
  private playbackStartedAt = Infinity;
  private playbackEpoch = 0;

  private constructor(context: AudioContext, sampleRate: number) {
    this.context = context;
    this.sourceSampleRate = sampleRate;
    this.analyser = context.createAnalyser();
    this.analyser.fftSize = 256;
    this.frequencyData = new Uint8Array(this.analyser.frequencyBinCount);
    this.analyser.connect(context.destination);
  }

  static create(sampleRate: number): RealtimePcmPlayer {
    const AudioContextCtor = window.AudioContext ?? window.webkitAudioContext;
    if (!AudioContextCtor) {
      throw new Error('AudioContext is unavailable');
    }
    // Match the duplex demo's persistent 24 kHz output stream. The microphone
    // recorder reuses this same context, avoiding the competing AudioContexts
    // that WKWebView may suspend when capture starts.
    let context: AudioContext;
    try {
      context = new AudioContextCtor({ sampleRate, latencyHint: 'interactive' });
    } catch {
      // Older WebKit builds may reject a requested rate. AudioBuffer retains
      // the provider's rate, so the context can still resample it to hardware.
      context = new AudioContextCtor({ latencyHint: 'interactive' });
    }
    const player = new RealtimePcmPlayer(context, sampleRate);
    player.unlockFromUserGesture();
    return player;
  }

  setSourceSampleRate(sampleRate: number): void {
    if (!Number.isFinite(sampleRate) || sampleRate <= 0) {
      throw new Error(`Invalid realtime audio sample rate: ${sampleRate}`);
    }
    this.sourceSampleRate = sampleRate;
  }

  getAudioContext(): AudioContext {
    return this.context;
  }

  /** Uses the output clock, so queued packets and provider completion are not speech. */
  isPlaying(): boolean {
    return this.context.state === 'running'
      && this.sources.size > 0
      && this.context.currentTime >= this.playbackStartedAt
      && this.context.currentTime < this.nextStartTime;
  }

  readFrequencyData(): Uint8Array<ArrayBuffer> | null {
    if (!this.isPlaying()) return null;
    this.analyser.getByteFrequencyData(this.frequencyData);
    return this.frequencyData;
  }

  private unlockFromUserGesture(): void {
    if (this.context.state !== 'running' && this.context.state !== 'closed') {
      void this.ensureRunning().catch(() => undefined);
    }

    // Starting a silent source while the click activation is still live keeps
    // WebKit from treating later streamed audio as autoplay.
    const buffer = this.context.createBuffer(1, 1, this.context.sampleRate);
    const source = this.context.createBufferSource();
    source.buffer = buffer;
    source.connect(this.context.destination);
    source.addEventListener('ended', () => source.disconnect(), { once: true });
    source.start();
  }

  private async ensureRunning(): Promise<void> {
    if (this.context.state === 'closed') {
      throw new Error('Realtime audio output is closed');
    }
    if (this.context.state === 'running') return;
    if (!this.resumeInFlight) {
      this.resumeInFlight = this.context.resume().finally(() => {
        this.resumeInFlight = null;
      });
    }
    await this.resumeInFlight;
    const resumedState = this.currentState();
    if (resumedState !== 'running') {
      throw new Error(`Realtime audio output did not start (${resumedState})`);
    }
  }

  private currentState(): AudioContextState {
    return this.context.state;
  }

  async enqueue(pcm16Base64: string): Promise<void> {
    const epoch = this.playbackEpoch;
    const samples = decodePcm16Base64(pcm16Base64);
    if (!samples.length || this.context.state === 'closed') {
      return;
    }
    await this.ensureRunning();
    if (epoch !== this.playbackEpoch || this.currentState() === 'closed') return;

    const buffer = this.context.createBuffer(1, samples.length, this.sourceSampleRate);
    buffer.copyToChannel(samples, 0);
    const source = this.context.createBufferSource();
    source.buffer = buffer;
    source.connect(this.analyser);
    // Keep a small lead just like the official browser demo so network-sized
    // PCM chunks form one continuous output stream without overlapping.
    const startAt = this.nextStartTime > this.context.currentTime
      ? this.nextStartTime
      : this.context.currentTime + 0.04;
    this.nextStartTime = startAt + buffer.duration;
    if (!this.sources.size) this.playbackStartedAt = startAt;
    this.sources.add(source);
    source.addEventListener('ended', () => {
      source.disconnect();
      this.sources.delete(source);
    }, { once: true });
    source.start(startAt);
  }

  stop(): void {
    this.playbackEpoch += 1;
    this.sources.forEach(source => {
      try {
        source.stop();
      } catch {
        // The source may have ended between iteration and stop().
      }
      source.disconnect();
    });
    this.sources.clear();
    this.nextStartTime = this.context.currentTime;
    this.playbackStartedAt = Infinity;
  }

  async close(): Promise<void> {
    this.stop();
    this.analyser.disconnect();
    if (this.context.state !== 'closed') {
      await this.context.close();
    }
  }
}
