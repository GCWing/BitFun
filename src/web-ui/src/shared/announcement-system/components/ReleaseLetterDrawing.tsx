import React, { useId } from 'react';

/** One vector is drawn, rounded, and then moved into the letter background. */
export default function ReleaseLetterDrawing() {
  const id = useId();
  return (
    <svg className="release-letter-drawing" id={`${id}-startupDrawing`} data-drawing="startupDrawing" viewBox="-64 -64 640 640" aria-hidden="true" focusable="false">
    <defs>
    <clipPath id={`${id}-startupRingClip`} data-drawing="startupRingClip">
    <path id={`${id}-startupClipPath`} data-drawing="startupClipPath" fillRule="evenodd" clipRule="evenodd"/>
    </clipPath>

    <linearGradient id={`${id}-startup-ceramic`} data-drawing="startup-ceramic" x1="0" y1="0" x2=".65" y2="1">
    <stop offset="0" stopColor="color-mix(in srgb, var(--openbitfun-color-surface-panel) 85%, var(--openbitfun-color-content-muted))"/>
    <stop offset=".52" stopColor="color-mix(in srgb, var(--openbitfun-color-surface-panel) 75%, var(--openbitfun-color-content-muted))"/>
    <stop offset="1" stopColor="color-mix(in srgb, var(--openbitfun-color-surface-panel) 60%, var(--openbitfun-color-content-muted))"/>
    </linearGradient>
    <radialGradient id={`${id}-startup-diffuse-light`} data-drawing="startup-diffuse-light" gradientUnits="userSpaceOnUse" cx="154" cy="108" r="370">
    <stop offset="0" stopColor="var(--openbitfun-color-content-on-dark)" stopOpacity=".58"/>
    <stop offset=".55" stopColor="var(--openbitfun-color-content-on-dark)" stopOpacity=".16"/>
    <stop offset="1" stopColor="var(--openbitfun-color-content-on-dark)" stopOpacity="0"/>
    </radialGradient>
    <linearGradient id={`${id}-startup-ceramic-edge`} data-drawing="startup-ceramic-edge" x1="0" y1="0" x2=".4" y2="1">
    <stop offset="0" stopColor="var(--openbitfun-color-content-on-dark)" stopOpacity=".72"/>
    <stop offset="1" stopColor="var(--openbitfun-color-content-muted)" stopOpacity=".35"/>
    </linearGradient>
    </defs>
    <g id={`${id}-guideField`} data-drawing="guideField">
    <g id={`${id}-axes`} data-drawing="axes" className="guide">
    <path data-axis="" pathLength="1" d="M256 256V-26"/>
    <path data-axis="" pathLength="1" d="M256 256H538"/>
    <path data-axis="" pathLength="1" d="M256 256V538"/>
    <path data-axis="" pathLength="1" d="M256 256H-26"/>
    </g>
    <g id={`${id}-circles`} data-drawing="circles" className="guide">
    <circle id={`${id}-outerCircle`} data-drawing="outerCircle" pathLength="1" cx="256" cy="256" r="214.197" transform="rotate(-90 256 256)"/>
    <circle id={`${id}-innerCircle`} data-drawing="innerCircle" pathLength="1" cx="256" cy="256" r="144" transform="rotate(-90 256 256)"/>
    <circle id={`${id}-radiusCircle`} data-drawing="radiusCircle" pathLength="1" cx="256" cy="256" r="60" transform="rotate(-90 256 256)"/>
    </g>
    <g id={`${id}-diagonals`} data-drawing="diagonals" className="guide-soft"><path d="M-14-14 526 526M-14 526 526-14"/></g>
    <g id={`${id}-bounds`} data-drawing="bounds" className="guide">
    <path data-bound="" pathLength="1" d="M70.5-26V538"/>
    <path data-bound="" pathLength="1" d="M441.5-26V538"/>
    <path data-bound="" pathLength="1" d="M-26 112H538"/>
    <path data-bound="" pathLength="1" d="M-26 400H538"/>
    </g>
    <g id={`${id}-radials`} data-drawing="radials" className="guide-soft"></g>
    <g id={`${id}-dimensionLines`} data-drawing="dimensionLines" className="guide">
    <path d="M70.5 495H441.5M70.5 491V499M441.5 491V499M480 112V400M476 112H484M476 400H484"/>
    </g>
    <g id={`${id}-dimensionLabels`} data-drawing="dimensionLabels" className="construction-label">
    <text x="256" y="511" textAnchor="middle">371</text>
    <text x="496" y="259" textAnchor="middle" transform="rotate(-90 496 259)">288</text>
    <text x="325" y="251">R60</text>
    <text x="285" y="229">30°</text>
    <path className="guide" d="M286 256A30 30 0 0 0 282 241M256 256H316"/>
    </g>
    </g>
    <g id={`${id}-filletGuides`} data-drawing="filletGuides"></g>
    <g id={`${id}-anchorNodes`} data-drawing="anchorNodes"></g>
    <g id={`${id}-formOutline`} data-drawing="formOutline" opacity="0">
    <path id={`${id}-outlineEdge`} data-drawing="outlineEdge" fill="none" stroke="var(--openbitfun-color-content-secondary)" strokeWidth="1.25" pathLength="1" fillRule="evenodd"/>
    </g>
    <g id={`${id}-material`} data-drawing="material" opacity="0">
    <g clip-path={`url(#${id}-startupRingClip)`}>
    <rect id={`${id}-materialFill`} data-drawing="materialFill" width="512" height="512" fill={`url(#${id}-startup-ceramic)`}/>
    <rect width="512" height="512" fill={`url(#${id}-startup-diffuse-light)`} opacity=".45"/>
    <rect id={`${id}-paperSurface`} data-drawing="paperSurface" width="512" height="512" fill="var(--openbitfun-color-surface-workbench)" opacity="0"/>
    </g>
    <path id={`${id}-materialRim`} data-drawing="materialRim" fill="none" stroke={`url(#${id}-startup-ceramic-edge)`} strokeWidth=".85" fillRule="evenodd"/>
    </g>
    <g id={`${id}-compassHeads`} data-drawing="compassHeads">
    <circle id={`${id}-penOuter`} data-drawing="penOuter" className="pen" r="2.1"/>
    <circle id={`${id}-penInner`} data-drawing="penInner" className="pen" r="1.8"/>
    <circle id={`${id}-penRadius`} data-drawing="penRadius" className="pen" r="1.5"/>
    </g>
    <circle id={`${id}-centerPoint`} data-drawing="centerPoint" className="pen" cx="256" cy="256" r="2"/>
    </svg>
  );
}
