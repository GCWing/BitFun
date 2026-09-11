import { useEffect, useRef, type CanvasHTMLAttributes } from "react";
import { classNames } from "../../internal/classNames";
import { createVoiceParticleRenderer } from "./voiceParticleRenderer";
import { SILENT_VOICE_AUDIO, type VoiceParticleAudioReader } from "./voiceParticleDynamics";
import styles from "./VoiceParticleLogo.module.css";

export interface VoiceParticleLogoProps extends CanvasHTMLAttributes<HTMLCanvasElement> {
  /** Read the current microphone/output FFT without publishing audio frames through React. */
  readAudio?: VoiceParticleAudioReader;
  active?: boolean;
}

/** Decorative, audio-reactive logo. The host alone owns media and permission lifetimes. */
export function VoiceParticleLogo({ readAudio, active = true, className, ...props }: VoiceParticleLogoProps) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const readerRef = useRef(readAudio);
  useEffect(() => { readerRef.current = readAudio; }, [readAudio]);

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    const motion = window.matchMedia("(prefers-reduced-motion: reduce)");
    const colors = getComputedStyle(canvas);
    let foreground = colors.color;
    let background = colors.getPropertyValue("--openbitfun-color-content-on-light").trim();
    const renderer = createVoiceParticleRenderer(
      canvas,
      foreground,
      background,
    );
    if (!renderer) return;

    let frame = 0;
    let lastFrame = 0;
    let visible = true;
    let width = 0;
    let height = 0;
    const frameDuration = 1000 / 60;
    const canAnimate = () => active && visible && !document.hidden && !motion.matches && width > 0 && height > 0;
    const animate = (time: number) => {
      frame = 0;
      if (!canAnimate()) return;
      const elapsed = time - lastFrame;
      if (elapsed >= frameDuration - 0.5) {
        lastFrame = time - (elapsed % frameDuration);
        renderer.draw(time, readerRef.current?.() ?? SILENT_VOICE_AUDIO);
      }
      frame = requestAnimationFrame(animate);
    };
    const sync = () => {
      cancelAnimationFrame(frame);
      frame = 0;
      lastFrame = 0;
      if (!width || !height) return;
      if (canAnimate()) frame = requestAnimationFrame(animate);
      else renderer.draw(0, SILENT_VOICE_AUDIO, false);
    };
    const resize = new ResizeObserver(([entry]) => {
      if (!entry) return;
      width = entry.contentRect.width;
      height = entry.contentRect.height;
      renderer.resize(width, height, window.devicePixelRatio || 1);
      sync();
    });
    const intersection = new IntersectionObserver(([entry]) => {
      if (!entry) return;
      visible = entry.isIntersecting;
      sync();
    });
    const theme = new MutationObserver(() => {
      const current = getComputedStyle(canvas);
      const nextForeground = current.color;
      const nextBackground = current.getPropertyValue("--openbitfun-color-content-on-light").trim();
      if (foreground === nextForeground && background === nextBackground) return;
      foreground = nextForeground;
      background = nextBackground;
      renderer.setColors(foreground, background);
      sync();
    });
    for (let ancestor: HTMLElement | null = canvas; ancestor; ancestor = ancestor.parentElement) {
      theme.observe(ancestor, { attributes: true, attributeFilter: ["style", "class", "data-color-scheme", "data-contrast"] });
    }
    const updateProjection = () => {
      renderer.resize(width, height, window.devicePixelRatio || 1);
      sync();
    };
    resize.observe(canvas);
    intersection.observe(canvas);
    motion.addEventListener("change", sync);
    document.addEventListener("visibilitychange", sync);
    window.addEventListener("resize", updateProjection);
    return () => {
      cancelAnimationFrame(frame);
      resize.disconnect();
      intersection.disconnect();
      theme.disconnect();
      motion.removeEventListener("change", sync);
      document.removeEventListener("visibilitychange", sync);
      window.removeEventListener("resize", updateProjection);
    };
  }, [active]);

  return <canvas {...props} ref={canvasRef} className={classNames(styles.root, className)}
    data-openbitfun-component="voice-particle-logo" data-openbitfun-part="root" aria-hidden="true" />;
}
