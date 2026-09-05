import { describe, expect, it } from 'vitest';
import { localLetterViewport, sampleHandoffBox, sampleMascot, transformPoint, type LetterRect } from './releaseLetterMotion';

describe('release letter choreography', () => {
  it('keeps the rigid rod attached to the deformed socket and the body grounded throughout every jump', () => {
    for (let frame = 0; frame <= 120; frame++) {
      const pose = sampleMascot(frame / 120);
      const socket = transformPoint(pose.deform, { x: 53, y: 59.8 });
      const rodRoot = transformPoint(pose.head, { x: 52.965, y: 59.8 });
      expect(rodRoot.x).toBeCloseTo(socket.x, 8);
      expect(rodRoot.y).toBeCloseTo(socket.y, 8);
      expect(Math.hypot(pose.head[0], pose.head[1])).toBeCloseTo(1, 8);
      const ground = Math.max(...[{ x: 0, y: 104.5 }, { x: 53, y: 132 }, { x: 106, y: 104.5 }].map(foot => transformPoint(pose.body, transformPoint(pose.deform, foot)).y));
      expect(ground).toBeCloseTo(132, 8);
    }
  });

  it('lands at the starting pose and moves the drawing into the background without overshoot', () => {
    const { alpha: initialAlpha, ...initial } = sampleMascot(0);
    const { alpha: finalAlpha, ...final } = sampleMascot(1);
    expect(initialAlpha).toBe(0);
    expect(finalAlpha).toBe(1);
    expect(final).toEqual(initial);
    const from = { left: 150, top: 100, width: 480 }, to = { left: -80, top: -120, width: 1224 };
    expect(sampleHandoffBox(0, from, to)).toEqual(from);
    expect(sampleHandoffBox(1, from, to)).toEqual(to);
    let previous = from.width;
    for (let frame = 0; frame <= 120; frame++) {
      const box = sampleHandoffBox(frame / 120, from, to);
      expect(box.width).toBeGreaterThanOrEqual(previous);
      expect(box.width).toBeLessThanOrEqual(to.width);
      previous = box.width;
    }
  });

  it('measures the same local geometry during and after the dialog entrance scale', () => {
    const viewport = { left: 0, top: 0, width: 1100.5, height: 880.25 };
    const host = { left: 33.015, top: -112.32, width: 1034.47, height: 1034.47 };
    for (const scale of [.9, .98, 1]) {
      const screenRect = (rect: LetterRect): LetterRect => ({
        left: rect.left * scale + 203.75,
        top: rect.top * scale + 108.125,
        width: rect.width * scale,
        height: rect.height * scale,
      });
      const local = localLetterViewport(screenRect(viewport), screenRect(host), host);
      expect(local.left).toBeCloseTo(-host.left, 9);
      expect(local.top).toBeCloseTo(-host.top, 9);
      expect(local.width).toBeCloseTo(viewport.width, 9);
      expect(local.height).toBeCloseTo(viewport.height, 9);
    }
  });

  it('makes the final local handoff rectangle equal the authored centered CSS background', () => {
    const from = { left: 256.75, top: 112.125, width: 484.5 };
    const hostWidth = 1034.47;
    const targetWidth = hostWidth * 640 / 544;
    const to = { left: (hostWidth - targetWidth) / 2, top: (hostWidth - targetWidth) / 2, width: targetWidth };
    const penultimate = sampleHandoffBox(.99999, from, to);
    expect(penultimate.left).toBeCloseTo(to.left, 8);
    expect(penultimate.top).toBeCloseTo(to.top, 8);
    expect(penultimate.width).toBeCloseTo(to.width, 8);
    const final = sampleHandoffBox(1, from, to);
    expect(final).toEqual(to);
    expect(final.left + final.width / 2).toBe(hostWidth / 2);
    expect(final.top + final.width / 2).toBe(hostWidth / 2);
  });
});
