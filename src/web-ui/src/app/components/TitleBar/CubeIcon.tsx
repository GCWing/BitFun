/**
 * CubeIcon - static cube icon.
 * Based on CubeLoading styles, without rotation.
 */

import React from 'react';
import './CubeIcon.scss';

interface CubeIconProps {
  size?: number;
  className?: string;
}

// 3x3x3 = 27 block positions
const BLOCKS = (() => {
  const arr = [];
  for (let x = -1; x <= 1; x++) {
    for (let y = -1; y <= 1; y++) {
      for (let z = -1; z <= 1; z++) {
        arr.push({ x, y, z });
      }
    }
  }
  return arr;
})();

export const CubeIcon: React.FC<CubeIconProps> = ({
  size = 28,
  className = '',
}) => {
  // Derive dimensions from size
  const unit = size / 3;
  const block = unit * 0.85;

  return (
    <div 
      className={`cube-icon ${className}`}
      style={{ 
        width: size, 
        height: size,
        perspective: size * 4,
      }}
    >
      <div className="cube-icon__rubiks">
        {BLOCKS.map(({ x, y, z }, i) => (
          <div
            key={i}
            className="cube-icon__block"
            style={{
              width: block,
              height: block,
              margin: -block / 2,
              transform: `translate3d(${unit * x}px, ${unit * -y}px, ${unit * z}px)`,
            }}
          >
            <div className="cube-icon__face cube-icon__face--front" style={{ transform: `translateZ(${block / 2}px)` }} />
            <div className="cube-icon__face cube-icon__face--back" style={{ transform: `translateZ(${-block / 2}px) rotateY(180deg)` }} />
            <div className="cube-icon__face cube-icon__face--top" style={{ transform: `translateY(${-block / 2}px) rotateX(90deg)` }} />
            <div className="cube-icon__face cube-icon__face--bottom" style={{ transform: `translateY(${block / 2}px) rotateX(-90deg)` }} />
            <div className="cube-icon__face cube-icon__face--right" style={{ transform: `translateX(${block / 2}px) rotateY(90deg)` }} />
            <div className="cube-icon__face cube-icon__face--left" style={{ transform: `translateX(${-block / 2}px) rotateY(-90deg)` }} />
          </div>
        ))}
      </div>
    </div>
  );
};

export default CubeIcon;
