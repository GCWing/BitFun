import { describe, expect, it } from 'vitest';
import { sampleHandoffBox, sampleMascot, transformPoint } from './releaseLetterMotion';

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
});
