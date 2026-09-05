import { useCallback, useLayoutEffect, useRef } from 'react';
import { isReducedMotionPreferred } from '@/shared/utils/motionPreference';
import { createLetterDrawing } from '../components/releaseLetterDrawingRenderer';
import {
  CHARACTER_TIMES, CONTENT_AT, HANDOFF_MS, INTRO_MS, LETTER_END, MASCOT_AT,
  MASCOT_MS, SIGNATURE_AT, clamp, formatMatrix, round, sampleHandoffBox, sampleMascot, segment,
  type LetterBox,
} from '../components/releaseLetterMotion';

export function useReleaseLetterMotion() {
  const sceneRef = useRef<HTMLDivElement>(null);
  const controls = useRef({ replay: () => {}, skip: () => {}, bounce: () => {} });

  useLayoutEffect(() => {
    const scene = sceneRef.current;
    if (!scene) return;
    const find = <T extends Element>(selector: string) => scene.querySelector<T>(selector)!;
    const drawing = createLetterDrawing(find<SVGSVGElement>('.release-letter-drawing'));
    const background = find<HTMLElement>('.release-letter__construction');
    const logo = find<HTMLElement>('.release-letter__drawing-box');
    const scroller = find<HTMLElement>('.release-letter-scroll');
    const wordmark = find<HTMLElement>('.release-letter__intro-wordmark');
    const brandRule = find<HTMLElement>('.release-letter__intro-rule');
    const status = find<HTMLElement>('.release-letter__intro-status');
    const phases = Array.from(scene.querySelectorAll<HTMLElement>('[data-phase]'));
    const progressBar = find<HTMLElement>('.release-letter__intro-progress');
    const skipButton = find<HTMLButtonElement>('.release-letter__skip');
    const versionButton = find<HTMLButtonElement>('.release-letter__version-mark');
    const signature = find<HTMLElement>('.release-letter__signature');
    const chars = Array.from(scene.querySelectorAll<HTMLElement>('[data-typing-char]'));
    const reveals = Array.from(scene.querySelectorAll<HTMLElement>('[data-reveal]'));
    const mascotButton = find<HTMLButtonElement>('.release-letter__mascot-button');
    const mascot = find<SVGSVGElement>('.release-letter__mascot');
    const parts = new Map(Array.from(mascot.querySelectorAll<SVGElement>('[data-mascot]')).map(node => [node.dataset.mascot!, node]));
    const media = window.matchMedia?.('(prefers-reduced-motion: reduce)');
    let reduced = isReducedMotionPreferred();
    let elapsed = reduced ? LETTER_END : 0;
    let bounceElapsed: number | null = null;
    let frame: number | null = null;
    let lastTimestamp: number | null = null;
    let disposed = false;
    let geometryDirty = true;
    let geometry: { from: LetterBox; to: LetterBox } | null = null;
    let lastTyped = -1;

    function measure() {
      const hostRect = background.getBoundingClientRect();
      const viewport = scene!.getBoundingClientRect();
      const mobile = window.innerWidth <= 760;
      const size = Math.min(mobile ? 410 : 520, window.innerWidth * (mobile ? .92 : .68), window.innerHeight * (mobile ? .62 : .66), viewport.width * .92, viewport.height * .66);
      const targetSize = hostRect.width * 640 / 544;
      geometry = {
        from: { left: viewport.left + (viewport.width - size) / 2 - hostRect.left,
          top: viewport.top + (viewport.height - size) / 2 - size * .04 - hostRect.top, width: size },
        to: { left: (hostRect.width - targetSize) / 2, top: (hostRect.height - targetSize) / 2, width: targetSize },
      };
      geometryDirty = false;
    }

    function renderMascot(progress: number, replaying: boolean) {
      const pose = sampleMascot(progress);
      (['lift', 'body', 'deform', 'head'] as const).forEach(name => parts.get(name)!.setAttribute('transform', formatMatrix(pose[name])));
      mascot.style.opacity = String(replaying ? 1 : pose.alpha);
      Object.entries(pose.shadow).forEach(([name, value]) => {
        parts.get(name === 'blur' ? 'blur' : 'shadow')!.setAttribute(name === 'blur' ? 'stdDeviation' : name, String(round(value)));
      });
      [['eyeOne', 70.437, 97.442], ['eyeTwo', 85.701, 89.155]].forEach(([name, x, y]) => {
        parts.get(name as string)!.setAttribute('transform', `translate(${round(pose.fx.gazeX)} ${round(pose.fx.gazeY)}) translate(${x} ${y}) scale(1 ${round(pose.fx.eyeOpen)}) translate(${-Number(x)} ${-Number(y)})`);
      });
      mascot.dataset.settled = String(progress >= 1);
    }

    function render() {
      const intro = clamp(elapsed / INTRO_MS);
      const handoff = clamp((elapsed - INTRO_MS) / HANDOFF_MS);
      scene!.dataset.motionState = elapsed < INTRO_MS ? 'intro' : handoff < 1 ? 'handoff' : elapsed < LETTER_END ? 'letter' : 'settled';
      scene!.dataset.reducedMotion = String(reduced);
      if (elapsed <= INTRO_MS) drawing.render(intro);
      else { drawing.render(1); drawing.handoff(handoff); }
      if (handoff < 1) {
        if (geometryDirty || !geometry) measure();
        const { from, to } = geometry!;
        const box = sampleHandoffBox(handoff, from, to);
        logo.style.left = `${from.left}px`;
        logo.style.top = `${from.top}px`;
        logo.style.width = `${from.width}px`;
        logo.style.transform = `translate(${box.left - from.left}px, ${box.top - from.top}px) scale(${box.width / from.width})`;
      } else {
        logo.removeAttribute('style');
      }
      const brand = segment(intro, .845, .99) * (1 - segment(handoff, 0, .22));
      wordmark.style.opacity = String(brand);
      wordmark.style.transform = `translate(-50%, ${round((1 - segment(intro, .845, .99)) * 10)}px)`;
      brandRule.style.opacity = String(brand * .72);
      status.style.opacity = String(1 - segment(handoff, 0, .22));
      progressBar.style.transform = `scaleX(${intro})`;
      const phase = [0, .14, .32, .51, .70, .76].filter(start => intro >= start).length - 1;
      phases.forEach((node, i) => { node.hidden = i !== phase; });
      reveals.forEach(node => {
        const start = CONTENT_AT + Number(node.dataset.reveal) * 115;
        const p = reduced ? 1 : segment(elapsed, start, start + 900);
        node.style.opacity = String(p);
        node.style.transform = `translateY(${round((1 - p) * 11)}px)`;
      });
      const contentReady = elapsed >= CONTENT_AT;
      scene!.dataset.contentReady = String(contentReady);
      // A focused skip action remains usable until focus leaves it.
      skipButton.hidden = handoff >= 1 && document.activeElement !== skipButton;
      versionButton.disabled = !contentReady;
      signature.style.opacity = elapsed >= SIGNATURE_AT ? '1' : '0';
      const typed = CHARACTER_TIMES.filter(at => elapsed >= at).length;
      if (typed !== lastTyped) {
        chars.forEach((node, i) => { node.dataset.typed = String(i < typed); node.dataset.cursor = String(!reduced && i === typed - 1); });
        lastTyped = typed;
      }
      const mascotVisible = elapsed >= MASCOT_AT;
      mascotButton.disabled = !mascotVisible;
      mascotButton.style.visibility = mascotVisible ? 'visible' : 'hidden';
      renderMascot(bounceElapsed === null ? clamp((elapsed - MASCOT_AT) / MASCOT_MS) : bounceElapsed / MASCOT_MS, bounceElapsed !== null);
    }

    function stop() {
      if (frame !== null) cancelAnimationFrame(frame);
      frame = null;
      lastTimestamp = null;
    }
    function start() {
      if (!disposed && !document.hidden && !reduced && frame === null && (elapsed < LETTER_END || bounceElapsed !== null)) {
        frame = requestAnimationFrame(tick);
      }
    }
    function tick(timestamp: number) {
      frame = null;
      const delta = lastTimestamp === null ? 0 : timestamp - lastTimestamp;
      lastTimestamp = timestamp;
      elapsed = Math.min(LETTER_END, elapsed + delta);
      if (bounceElapsed !== null) bounceElapsed = Math.min(MASCOT_MS, bounceElapsed + delta);
      render();
      if (bounceElapsed === MASCOT_MS) bounceElapsed = null;
      if (elapsed === LETTER_END && bounceElapsed === null) lastTimestamp = null;
      start();
    }
    const skip = () => {
      stop();
      elapsed = LETTER_END;
      bounceElapsed = null;
      render();
    };
    controls.current = {
      skip,
      replay() {
        stop();
        scroller.scrollTop = 0;
        elapsed = reduced ? LETTER_END : 0;
        bounceElapsed = null;
        geometryDirty = true;
        lastTyped = -1;
        // Move focus before the version button is hidden by the intro.
        if (!reduced) { skipButton.hidden = false; skipButton.focus({ preventScroll: true }); }
        render();
        start();
      },
      bounce() {
        // Stable hit target; crossing the moving SVG never restarts a jump mid-air.
        if (reduced || elapsed < LETTER_END || bounceElapsed !== null) return;
        bounceElapsed = 0;
        render();
        start();
      },
    };
    const visibilityChanged = () => {
      scene.dataset.paused = String(document.hidden);
      if (document.hidden) stop(); else start();
    };
    const motionChanged = () => {
      reduced = media?.matches ?? false;
      if (reduced) { lastTyped = -1; skip(); } else render();
    };
    const resized = () => { geometryDirty = true; if (elapsed < INTRO_MS + HANDOFF_MS) render(); };
    const observer = typeof ResizeObserver === 'function' ? new ResizeObserver(resized) : null;
    observer?.observe(scene);
    observer?.observe(background);
    window.addEventListener('resize', resized);
    document.addEventListener('visibilitychange', visibilityChanged);
    media?.addEventListener('change', motionChanged);
    render();
    visibilityChanged();
    return () => {
      disposed = true;
      stop();
      observer?.disconnect();
      window.removeEventListener('resize', resized);
      document.removeEventListener('visibilitychange', visibilityChanged);
      media?.removeEventListener('change', motionChanged);
      controls.current = { replay() {}, skip() {}, bounce() {} };
    };
  }, []);

  return {
    sceneRef,
    replay: useCallback(() => controls.current.replay(), []),
    skip: useCallback(() => controls.current.skip(), []),
    bounce: useCallback(() => controls.current.bounce(), []),
  };
}
