import { useEffect, useRef } from 'react';

const CONTOUR_COUNT = 15;
const FLOW_CYCLE_MS = 18000;
const FLOW_LAYERS = [
  { length: 34, opacity: 0.12 },
  { length: 26, opacity: 0.14 },
  { length: 18, opacity: 0.36 },
];

function createContour(index: number): string {
  const progress = index / (CONTOUR_COUNT - 1);
  const radius = 79 + 23 * progress;
  // Preserve the original ribbon: flat inner edge, pointed outer crown.
  const angle = (-60 - 30 * progress) * Math.PI / 180;
  const vertices = Array.from({ length: 6 }, (_, vertex) => {
    const theta = angle + vertex * Math.PI / 3;
    return [128 + radius * Math.cos(theta), 128 + radius * Math.sin(theta)];
  });
  const corners = vertices.map((vertex, i) => {
    const previous = vertices[(i + 5) % 6];
    const next = vertices[(i + 1) % 6];
    return {
      entry: vertex.map((value, axis) => value + (previous[axis] - value) * 0.16),
      exit: vertex.map((value, axis) => value + (next[axis] - value) * 0.16),
      vertex,
    };
  });
  const point = (values: number[]) => values.map(value => value.toFixed(3)).join(' ');
  return corners.map((corner, i) =>
    `${i === 0 ? 'M' : 'L'} ${point(corner.entry)} Q ${point(corner.vertex)} ${point(corner.exit)}`,
  ).join(' ') + ' Z';
}

const contours = Array.from({ length: CONTOUR_COUNT }, (_, index) => createContour(index));

export function AboutBrandMark({ active = true }: { active?: boolean }) {
  const svgRef = useRef<SVGSVGElement>(null);

  useEffect(() => {
    const svg = svgRef.current;
    if (!svg || !active) return;
    const reducedMotion = window.matchMedia('(prefers-reduced-motion: reduce)');
    let animations: Animation[] = [];

    const synchronizePlayback = () => {
      if (reducedMotion.matches) {
        animations.forEach(animation => animation.cancel());
        animations = [];
        return;
      }
      if (document.hidden) {
        animations.forEach(animation => animation.pause());
        return;
      }
      if (animations.length === 0) {
        // Equal lap times preserve the spacing between highlights indefinitely.
        // Only the highlights circulate; the contours remain fixed.
        animations = Array.from(svg.querySelectorAll('.openbitfun-about-dialog__brand-flow'))
          .map((strand, index) => {
            const start = -index * 3;
            return strand.animate([
              { strokeDashoffset: String(start) },
              { strokeDashoffset: String(start - 100) },
            ], { duration: FLOW_CYCLE_MS, iterations: Infinity, easing: 'linear' });
          });
      } else {
        animations.forEach(animation => animation.play());
      }
    };

    synchronizePlayback();
    reducedMotion.addEventListener('change', synchronizePlayback);
    document.addEventListener('visibilitychange', synchronizePlayback);
    return () => {
      animations.forEach(animation => animation.cancel());
      reducedMotion.removeEventListener('change', synchronizePlayback);
      document.removeEventListener('visibilitychange', synchronizePlayback);
    };
  }, [active]);

  return (
    <svg
      ref={svgRef}
      className="openbitfun-about-dialog__brand-mark"
      viewBox="0 0 256 256"
      width={256}
      height={256}
      fill="none"
      stroke="currentColor"
      aria-hidden="true"
      focusable="false"
    >
      {contours.map((path, index) => (
        <g key={index}>
          <path d={path} opacity={index === 0 || index === CONTOUR_COUNT - 1 ? 0.58 : 0.34} />
          <g className="openbitfun-about-dialog__brand-flow" strokeDashoffset={-index * 3}>
            {FLOW_LAYERS.map(layer => (
              <path
                key={layer.length}
                d={path}
                pathLength={100}
                strokeDasharray={`${layer.length / 2} ${100 - layer.length} ${layer.length / 2} 0`}
                opacity={layer.opacity}
              />
            ))}
          </g>
        </g>
      ))}
    </svg>
  );
}
