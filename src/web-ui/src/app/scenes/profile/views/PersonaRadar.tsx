import React, { useMemo } from 'react';

export interface RadarDim {
  label: string;
  labelSub?: string;
  value: number; // 0–10
}

interface Props {
  dims: RadarDim[];
  size?: number;
  onDimClick?: (label: string) => void;
  onChartClick?: () => void;
}

const RING_FRACTIONS = [0.25, 0.5, 0.75, 1.0];

export const PersonaRadar: React.FC<Props> = ({ dims, size = 148, onDimClick, onChartClick }) => {
  const n = dims.length;
  const cx = size / 2;
  const cy = size / 2;
  const r = size * 0.33;
  const labelR = size * 0.47;

  const angles = useMemo(
    () => Array.from({ length: n }, (_, i) => -Math.PI / 2 + (i * 2 * Math.PI) / n),
    [n],
  );

  const pt = (angle: number, radius: number) => ({
    x: cx + radius * Math.cos(angle),
    y: cy + radius * Math.sin(angle),
  });

  const ringPaths = RING_FRACTIONS.map(frac =>
    angles
      .map((a, i) => {
        const p = pt(a, r * frac);
        return `${i === 0 ? 'M' : 'L'}${p.x.toFixed(2)},${p.y.toFixed(2)}`;
      })
      .join(' ') + ' Z',
  );

  const scorePath =
    angles
      .map((a, i) => {
        const norm = Math.min(1, Math.max(0, dims[i].value / 10));
        const p = pt(a, r * norm);
        return `${i === 0 ? 'M' : 'L'}${p.x.toFixed(2)},${p.y.toFixed(2)}`;
      })
      .join(' ') + ' Z';

  const dots = angles.map((a, i) => {
    const norm = Math.min(1, Math.max(0, dims[i].value / 10));
    return pt(a, r * norm);
  });

  const vertices = angles.map(a => pt(a, r));

  // Label hit areas: centered rect behind each label
  const labelPositions = angles.map((a, i) => {
    const lp = pt(a, labelR);
    const cosA = Math.cos(a);
    let anchor: 'middle' | 'start' | 'end' = 'middle';
    if (cosA > 0.15) anchor = 'start';
    else if (cosA < -0.15) anchor = 'end';
    return { ...lp, anchor, label: dims[i].label };
  });

  return (
    <svg
      width={size}
      height={size}
      viewBox={`0 0 ${size} ${size}`}
      className="bp-radar"
      aria-hidden="true"
    >
      {/* Grid rings */}
      {ringPaths.map((d, i) => (
        <path
          key={`ring-${i}`}
          d={d}
          style={{
            fill: i === RING_FRACTIONS.length - 1 ? 'var(--element-bg-subtle)' : 'none',
            stroke: 'var(--border-subtle)',
            strokeWidth: 0.7,
          }}
        />
      ))}

      {/* Axis spokes */}
      {vertices.map((v, i) => (
        <line
          key={`axis-${i}`}
          x1={cx} y1={cy} x2={v.x} y2={v.y}
          style={{ stroke: 'var(--border-subtle)', strokeWidth: 0.7 }}
        />
      ))}

      {/* Score fill */}
      <path
        d={scorePath}
        style={{
          fill: 'var(--color-accent-500)',
          fillOpacity: 0.12,
          stroke: 'var(--color-accent-500)',
          strokeWidth: 1.2,
          strokeLinejoin: 'round',
        }}
      />

      {/* Score dots */}
      {dots.map((p, i) => (
        <circle key={`dot-${i}`} cx={p.x} cy={p.y} r={2.2} style={{ fill: 'var(--color-accent-500)' }} />
      ))}

      {/* Chart hit area: only this area triggers "expand" */}
      {onChartClick && (
        <circle
          cx={cx}
          cy={cy}
          r={r + 6}
          fill="transparent"
          style={{ cursor: 'pointer' }}
          onClick={onChartClick}
        />
      )}

      {/* Labels (clickable if onDimClick provided) */}
      {labelPositions.map((lp, i) => {
        const hasClick = !!onDimClick;
        return (
          <g key={`label-${i}`}>
            {hasClick && (
              <rect
                x={lp.anchor === 'start' ? lp.x - 2 : lp.anchor === 'end' ? lp.x - 44 : lp.x - 23}
                y={lp.y - 7}
                width={46}
                height={14}
                fill="transparent"
                style={{ cursor: 'pointer' }}
                onClick={() => onDimClick(lp.label)}
              />
            )}
            <text
              x={lp.x.toFixed(2)}
              y={lp.y.toFixed(2)}
              textAnchor={lp.anchor}
              dominantBaseline="middle"
              style={{
                fontSize: '9px',
                fill: hasClick ? 'var(--color-text-secondary)' : 'var(--color-text-muted)',
                fontFamily: 'var(--font-family-sans)',
                userSelect: 'none',
                cursor: hasClick ? 'pointer' : 'default',
                transition: 'fill 0.15s',
              }}
              onClick={hasClick ? () => onDimClick(lp.label) : undefined}
            >
              {dims[i].label}
            </text>
          </g>
        );
      })}
    </svg>
  );
};
