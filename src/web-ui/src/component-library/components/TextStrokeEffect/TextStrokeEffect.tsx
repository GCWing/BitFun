"use client";

import React from "react";
import "./TextStrokeEffect.scss";

import {
  TEXT_STROKE_GRADIENT_COLORS,
  TEXT_STROKE_GRADIENT_OFFSETS,
  buildTextStrokeColorCycle,
} from './TextStrokeEffectGradient';

export interface TextStrokeEffectProps {
  text: string;
  duration?: number;
  className?: string;
  height?: string;
}

/**
 * Text stroke loop animation component
 * Pure CSS implementation, no extra animation libraries
 */
export const TextStrokeEffect: React.FC<TextStrokeEffectProps> = ({
  text,
  duration = 4,
  className = "",
  height = "100px",
}) => {
  const charWidth = 55;
  const viewBoxWidth = text.length * charWidth;
  const viewBoxHeight = 100;

  return (
    <svg
      className={`text-stroke-effect ${className}`}
      data-bf-component="text-stroke-effect"
      data-bf-part="root"
      viewBox={`0 0 ${viewBoxWidth} ${viewBoxHeight}`}
      xmlns="http://www.w3.org/2000/svg"
      style={{ 
        height: height,
        width: 'auto',
        display: 'block',
      }}
      preserveAspectRatio="xMidYMid meet"
    >
      <defs>
        <linearGradient id="textStrokeGradient" x1="0%" y1="0%" x2="100%" y2="0%">
          {TEXT_STROKE_GRADIENT_COLORS.map((color, index) => (
            <stop
              key={color}
              offset={TEXT_STROKE_GRADIENT_OFFSETS[index]}
              stopColor={color}
            >
              <animate
                attributeName="stop-color"
                values={buildTextStrokeColorCycle(index)}
                dur={`${duration}s`}
                repeatCount="indefinite"
              />
            </stop>
          ))}
        </linearGradient>
      </defs>

      <text
        x="50%"
        y="55%"
        textAnchor="middle"
        dominantBaseline="middle"
        className="text-stroke-effect__outline"
        data-bf-component="text-stroke-effect"
        data-bf-part="outline"
      >
        {text}
      </text>

      <text
        x="50%"
        y="55%"
        textAnchor="middle"
        dominantBaseline="middle"
        className="text-stroke-effect__animated"
        data-bf-component="text-stroke-effect"
        data-bf-part="animated"
        style={{
          animationDuration: `${duration}s`,
        }}
      >
        {text}
      </text>

      <text
        x="50%"
        y="55%"
        textAnchor="middle"
        dominantBaseline="middle"
        className="text-stroke-effect__gradient"
        data-bf-component="text-stroke-effect"
        data-bf-part="gradient"
        stroke="url(#textStrokeGradient)"
      >
        {text}
      </text>
    </svg>
  );
};

export default TextStrokeEffect;
