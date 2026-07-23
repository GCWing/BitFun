/**
 * 3D cube logo component, pure CSS 3D implementation
 */

import React, { useMemo } from 'react';
import './CubeLogo.scss';

export type CubeLogoVariant = 'default' | 'compact';

export interface CubeLogoProps {
  /** Cube size (px) */
  size?: number;
  /** Custom class name */
  className?: string;
  /** Whether to show particle effects */
  showParticles?: boolean;
  /** Variant: default - full | compact - compact (for small sizes) */
  variant?: CubeLogoVariant;
}

interface Particle {
  id: number;
  x: number;
  y: number;
  size: number;
  duration: number;
  delay: number;
  opacity: number;
}

export const CubeLogo: React.FC<CubeLogoProps> = ({ 
  size = 100, 
  className = '', 
  showParticles = true,
  variant = 'default'
}) => {
  const effectiveVariant = variant === 'default' && size < 50 ? 'compact' : variant;
  const isCompact = effectiveVariant === 'compact';

  const blockSize = size / 4;
  const gap = blockSize * 0.12;
  const actualSize = blockSize - gap;

  const particles = useMemo<Particle[]>(() => {
    if (!showParticles) return [];
    const count = 12;
    return Array.from({ length: count }, (_, i) => ({
      id: i,
      x: Math.random() * 140 - 20,
      y: Math.random() * 140 - 20,
      size: Math.random() * 3 + 1,
      duration: Math.random() * 3 + 4,
      delay: Math.random() * 4,
      opacity: Math.random() * 0.5 + 0.2,
    }));
  }, [showParticles]);

  const blocks = useMemo(() => {
    const result = [];
    for (let x = -1; x <= 1; x++) {
      for (let y = -1; y <= 1; y++) {
        for (let z = -1; z <= 1; z++) {
          result.push({ x, y, z, key: `${x}-${y}-${z}` });
        }
      }
    }
    return result;
  }, []);

  const renderBlock = (x: number, y: number, z: number, key: string) => {
    return (
      <div
        key={key}
        className="cube-logo__block"
        style={{
          width: actualSize,
          height: actualSize,
          transform: `translate3d(${x * blockSize}px, ${-y * blockSize}px, ${z * blockSize}px)`
        }}
      >
        <div className="cube-logo__face cube-logo__face--front" style={{ width: actualSize, height: actualSize, transform: `translateZ(${actualSize/2}px)` }} />
        <div className="cube-logo__face cube-logo__face--back" style={{ width: actualSize, height: actualSize, transform: `translateZ(${-actualSize/2}px) rotateY(180deg)` }} />
        <div className="cube-logo__face cube-logo__face--top" style={{ width: actualSize, height: actualSize, transform: `translateY(${-actualSize/2}px) rotateX(90deg)` }} />
        <div className="cube-logo__face cube-logo__face--bottom" style={{ width: actualSize, height: actualSize, transform: `translateY(${actualSize/2}px) rotateX(-90deg)` }} />
        <div className="cube-logo__face cube-logo__face--right" style={{ width: actualSize, height: actualSize, transform: `translateX(${actualSize/2}px) rotateY(90deg)` }} />
        <div className="cube-logo__face cube-logo__face--left" style={{ width: actualSize, height: actualSize, transform: `translateX(${-actualSize/2}px) rotateY(-90deg)` }} />
      </div>
    );
  };

  return (
    <div 
      className={`cube-logo ${isCompact ? 'cube-logo--compact' : ''} ${className}`}
      style={{ 
        width: size, 
        height: size,
        perspective: size * 3
      }}
    >
      {showParticles && (
        <div className="cube-logo__particles">
          {particles.map(p => (
            <span
              key={p.id}
              className="cube-logo__particle"
              style={{
                left: `${p.x}%`,
                top: `${p.y}%`,
                width: p.size,
                height: p.size,
                opacity: p.opacity,
                animationDuration: `${p.duration}s`,
                animationDelay: `${p.delay}s`,
              }}
            />
          ))}
        </div>
      )}
      
      <div className="cube-logo__scene">
        <div className="cube-logo__cube">
          {blocks.map(({ x, y, z, key }) => renderBlock(x, y, z, key))}
        </div>
      </div>
    </div>
  );
};

CubeLogo.displayName = 'CubeLogo';

export default CubeLogo;
