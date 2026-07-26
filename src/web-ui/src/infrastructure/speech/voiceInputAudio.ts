export interface VoiceInputRecorder {
  stop: () => Promise<void>;
}

export interface VoiceInputMicrophone {
  deviceId: string;
  label: string;
}

export type VoiceInputRecorderStartupStage = 'media-stream' | 'audio-graph';

export interface VoiceInputRecorderStartupTiming {
  stage: VoiceInputRecorderStartupStage;
  elapsedMs: number;
  totalElapsedMs: number;
}

export interface VoiceInputRecorderOptions {
  targetSampleRate: number;
  chunkDurationMs: number;
  microphoneDeviceId?: string;
  onChunk: (pcm16Base64: string) => void;
  onLevel?: (level: number) => void;
  onDeviceEnded?: () => void;
  onStartupTiming?: (timing: VoiceInputRecorderStartupTiming) => void;
}

const PCM_BYTES_PER_SAMPLE = 2;

export async function listVoiceInputMicrophones(): Promise<VoiceInputMicrophone[]> {
  if (!navigator.mediaDevices?.enumerateDevices) {
    return [];
  }
  const devices = await navigator.mediaDevices.enumerateDevices();
  return devices
    .filter(device => device.kind === 'audioinput')
    .map(device => ({ deviceId: device.deviceId, label: device.label }));
}

function encodePcm16Base64(samples: Float32Array): string {
  const bytes = new Uint8Array(samples.length * PCM_BYTES_PER_SAMPLE);
  const view = new DataView(bytes.buffer);
  for (let i = 0; i < samples.length; i += 1) {
    const clamped = Math.max(-1, Math.min(1, samples[i]));
    const pcm = clamped < 0 ? clamped * 0x8000 : clamped * 0x7fff;
    view.setInt16(i * PCM_BYTES_PER_SAMPLE, pcm, true);
  }

  let binary = '';
  for (let i = 0; i < bytes.length; i += 1) {
    binary += String.fromCharCode(bytes[i]);
  }
  return window.btoa(binary);
}

function resampleLinear(input: Float32Array, sourceSampleRate: number, targetSampleRate: number): Float32Array {
  if (sourceSampleRate === targetSampleRate) {
    return input;
  }
  const ratio = sourceSampleRate / targetSampleRate;
  const outputLength = Math.max(1, Math.floor(input.length / ratio));
  const output = new Float32Array(outputLength);

  for (let i = 0; i < outputLength; i += 1) {
    const sourceIndex = i * ratio;
    const left = Math.floor(sourceIndex);
    const right = Math.min(input.length - 1, left + 1);
    const weight = sourceIndex - left;
    output[i] = input[left] * (1 - weight) + input[right] * weight;
  }

  return output;
}

function appendSamples(buffer: Float32Array, next: Float32Array): Float32Array {
  if (buffer.length === 0) {
    return next;
  }
  const merged = new Float32Array(buffer.length + next.length);
  merged.set(buffer, 0);
  merged.set(next, buffer.length);
  return merged;
}

function calculateRmsLevel(samples: Float32Array): number {
  if (samples.length === 0) {
    return 0;
  }

  let sum = 0;
  for (let i = 0; i < samples.length; i += 1) {
    sum += samples[i] * samples[i];
  }

  return Math.min(1, Math.sqrt(sum / samples.length) * 5);
}

export async function createVoiceInputRecorder({
  targetSampleRate,
  chunkDurationMs,
  microphoneDeviceId,
  onChunk,
  onLevel,
  onDeviceEnded,
  onStartupTiming,
}: VoiceInputRecorderOptions): Promise<VoiceInputRecorder> {
  if (!navigator.mediaDevices?.getUserMedia) {
    throw new Error('Microphone capture is unavailable');
  }

  const startupStartedAt = performance.now();
  const mediaStream = await navigator.mediaDevices.getUserMedia({
    audio: {
      channelCount: 1,
      echoCancellation: true,
      noiseSuppression: true,
      autoGainControl: true,
      ...(microphoneDeviceId ? { deviceId: { exact: microphoneDeviceId } } : {}),
    },
  });
  const mediaStreamReadyAt = performance.now();
  onStartupTiming?.({
    stage: 'media-stream',
    elapsedMs: Math.round(mediaStreamReadyAt - startupStartedAt),
    totalElapsedMs: Math.round(mediaStreamReadyAt - startupStartedAt),
  });

  const AudioContextCtor = window.AudioContext ?? window.webkitAudioContext;
  if (!AudioContextCtor) {
    mediaStream.getTracks().forEach(track => track.stop());
    throw new Error('AudioContext is unavailable');
  }

  let audioContext: AudioContext | null = null;
  let source: MediaStreamAudioSourceNode | null = null;
  let processor: ScriptProcessorNode | null = null;
  let pending = new Float32Array(0);
  let stopped = false;

  const stop = async () => {
    if (stopped) return;
    stopped = true;
    if (processor) {
      processor.disconnect();
      processor.onaudioprocess = null;
    }
    source?.disconnect();
    if (pending.length > 0) {
      onChunk(encodePcm16Base64(pending));
      pending = new Float32Array(0);
    }
    mediaStream.getTracks().forEach(track => track.stop());
    if (audioContext && audioContext.state !== 'closed') {
      await audioContext.close();
    }
  };

  try {
    audioContext = new AudioContextCtor();
    source = audioContext.createMediaStreamSource(mediaStream);
    processor = audioContext.createScriptProcessor(4096, 1, 1);
    const chunkSize = Math.max(1, Math.floor(targetSampleRate * (chunkDurationMs / 1000)));

    processor.onaudioprocess = (event) => {
      const input = event.inputBuffer.getChannelData(0);
      onLevel?.(calculateRmsLevel(input));

      const resampled = resampleLinear(input, audioContext!.sampleRate, targetSampleRate);
      pending = appendSamples(pending, resampled);

      while (pending.length >= chunkSize) {
        const chunk = pending.slice(0, chunkSize);
        pending = pending.slice(chunkSize);
        onChunk(encodePcm16Base64(chunk));
      }
    };

    for (const track of mediaStream.getAudioTracks()) {
      track.addEventListener('ended', () => {
        if (!stopped) onDeviceEnded?.();
      }, { once: true });
    }

    source.connect(processor);
    processor.connect(audioContext.destination);
    const audioGraphReadyAt = performance.now();
    onStartupTiming?.({
      stage: 'audio-graph',
      elapsedMs: Math.round(audioGraphReadyAt - mediaStreamReadyAt),
      totalElapsedMs: Math.round(audioGraphReadyAt - startupStartedAt),
    });
  } catch (error) {
    await stop().catch(() => undefined);
    throw error;
  }

  return { stop };
}

declare global {
  interface Window {
    webkitAudioContext?: typeof AudioContext;
  }
}
