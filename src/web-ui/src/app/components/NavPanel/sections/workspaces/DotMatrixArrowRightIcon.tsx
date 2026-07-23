import React from 'react';

/**
 * Right-pointing filled triangle rendered as a 3-column × 5-row dot grid (▶ style).
 *
 * Grid (col 0 = left, col 2 = tip on the right):
 *   •  .  .
 *   •  •  .
 *   •  •  •   ← rightmost tip
 *   •  •  .
 *   •  .  .
 */
export const DotMatrixArrowRightIcon: React.FC<{
  size?: number;
  className?: string;
}> = ({ size = 14, className }) => {
  const rows = [
    '100',
    '110',
    '111',
    '110',
    '100',
  ];

  const cell = 4;
  const dotR = 1.5;
  const w = rows[0].length * cell;
  const h = rows.length * cell;

  const dots: React.ReactNode[] = [];
  rows.forEach((row, y) => {
    for (let x = 0; x < row.length; x += 1) {
      if (row[x] !== '1') continue;
      dots.push(
        <circle
          key={`${x}-${y}`}
          cx={x * cell + cell / 2}
          cy={y * cell + cell / 2}
          r={dotR}
        />
      );
    }
  });

  return (
    <svg
      width={size}
      height={size}
      viewBox={`0 0 ${w} ${h}`}
      className={className}
      aria-hidden="true"
    >
      <g fill="currentColor">{dots}</g>
    </svg>
  );
};
