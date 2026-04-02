/**
 * BitFun pixel panda — 32×32, white halo + ear rims for dark UI,
 * softer patches and dot eyes for silly-cute (蠢萌) read.
 */

import React from 'react';
import type { ChatInputPetMood } from '../utils/chatInputPetMood';
import './ChatInputPixelPet.scss';

export interface ChatInputPixelPetProps {
  mood: ChatInputPetMood;
  className?: string;
  layout?: 'center' | 'stopRight';
}

const MOODS: ChatInputPetMood[] = ['rest', 'analyzing', 'waiting', 'working'];

const VIEW = 32;

/** Cream face — slightly rounded; bottom rows taper (no “neck” block). */
const CREAM_ROWS: [number, number, number][] = [
  [5, 10, 12],
  [6, 8, 16],
  [8, 6, 20],
  [10, 5, 22],
  [12, 4, 24],
  [14, 4, 24],
  [16, 4, 24],
  [18, 4, 24],
  [20, 4, 24],
  [22, 5, 22],
  [24, 6, 20],
  [25, 8, 16],
  [26, 10, 12],
];

/**
 * White rim only: pad each cream row + extra top rows for ears.
 * No extra full-width rows below the face (avoids “pedestal” look).
 */
function buildHaloRows(pad: number): [number, number, number][] {
  const out: [number, number, number][] = [];
  const first = CREAM_ROWS[0];
  out.push([first[0] - 2, first[1] - pad, first[2] + 2 * pad]);
  out.push([first[0] - 1, first[1] - pad, first[2] + 2 * pad]);
  for (const r of CREAM_ROWS) {
    out.push([r[0], r[1] - pad, r[2] + 2 * pad]);
  }
  return out;
}

const HALO_ROWS = buildHaloRows(3);

/** Left / right ear — black pixels (drawn after white ear-rim). */
const EAR_LEFT: [number, number, number, number][] = [
  [2, 5, 4, 2],
  [3, 4, 6, 2],
  [4, 4, 6, 2],
];
const EAR_RIGHT: [number, number, number, number][] = [
  [2, 23, 4, 2],
  [3, 22, 6, 2],
  [4, 22, 6, 2],
];

/** White under ears so black ears read on dark backgrounds. */
const EAR_RIM_LEFT: [number, number, number, number][] = [
  [1, 4, 6, 4],
  [2, 3, 8, 4],
  [3, 3, 8, 4],
  [4, 3, 8, 4],
];
const EAR_RIM_RIGHT: [number, number, number, number][] = [
  [1, 22, 6, 4],
  [2, 21, 8, 4],
  [3, 21, 8, 4],
  [4, 21, 8, 4],
];

/** Irregular oval-ish patches (less “gas mask” than big rectangles). */
const PATCH_LEFT: [number, number, number, number][] = [
  [9, 5, 6, 2],
  [10, 4, 8, 2],
  [11, 3, 10, 6],
  [12, 3, 10, 6],
  [13, 4, 8, 2],
  [14, 5, 6, 2],
];
const PATCH_RIGHT: [number, number, number, number][] = [
  [9, 19, 6, 2],
  [10, 18, 8, 2],
  [11, 17, 10, 6],
  [12, 17, 10, 6],
  [13, 18, 8, 2],
  [14, 19, 6, 2],
];

function Px({
  x,
  y,
  w = 1,
  h = 1,
  className,
}: {
  x: number;
  y: number;
  w?: number;
  h?: number;
  className?: string;
}) {
  return <rect x={x} y={y} width={w} height={h} className={className} />;
}

/** Slight overlap removes 1px scanline gaps when scaled. */
function PxRow({ x, y, w, className }: { x: number; y: number; w: number; className?: string }) {
  return <rect x={x} y={y - 0.01} width={w} height={1.02} className={className} />;
}

function HeadHalo() {
  const cls = 'bitfun-panda__halo';
  return (
    <g className="bitfun-panda-head__halo">
      {HALO_ROWS.map(([y, x0, lw], i) => (
        <PxRow key={`h-${i}`} x={x0} y={y} w={lw} className={cls} />
      ))}
    </g>
  );
}

function HeadDisk() {
  const cls = 'bitfun-panda__w';
  return (
    <g className="bitfun-panda-head__disk">
      {CREAM_ROWS.map(([y, x0, lw], i) => (
        <PxRow key={`c-${i}`} x={x0} y={y} w={lw} className={cls} />
      ))}
    </g>
  );
}

function EarRims() {
  const cls = 'bitfun-panda__halo';
  return (
    <g className="bitfun-panda-head__ear-rims" aria-hidden>
      {EAR_RIM_LEFT.map(([y, x, w, h], i) => (
        <Px key={`el-${i}`} x={x} y={y} w={w} h={h} className={cls} />
      ))}
      {EAR_RIM_RIGHT.map(([y, x, w, h], i) => (
        <Px key={`er-${i}`} x={x} y={y} w={w} h={h} className={cls} />
      ))}
    </g>
  );
}

function Ears() {
  const b = 'bitfun-panda__b';
  return (
    <g className="bitfun-panda-head__ears" aria-hidden>
      {EAR_LEFT.map(([y, x, w, h], i) => (
        <Px key={`eL-${i}`} x={x} y={y} w={w} h={h} className={b} />
      ))}
      {EAR_RIGHT.map(([y, x, w, h], i) => (
        <Px key={`eR-${i}`} x={x} y={y} w={w} h={h} className={b} />
      ))}
    </g>
  );
}

function EyePatches() {
  const b = 'bitfun-panda__b';
  return (
    <g className="bitfun-panda-head__patches" aria-hidden>
      {PATCH_LEFT.map(([y, x, w, h], i) => (
        <Px key={`pl-${i}`} x={x} y={y} w={w} h={h} className={b} />
      ))}
      {PATCH_RIGHT.map(([y, x, w, h], i) => (
        <Px key={`pr-${i}`} x={x} y={y} w={w} h={h} className={b} />
      ))}
    </g>
  );
}

/** Small inverted-T nose — cute button, not wide bar. */
function Nose() {
  const b = 'bitfun-panda__b';
  return (
    <g className="bitfun-panda-head__nose" aria-hidden>
      <Px x={15} y={21} w={2} h={1} className={b} />
      <Px x={14} y={22} w={4} h={1} className={b} />
    </g>
  );
}

/** Open: chunky whites + 1px pupils (offset slightly — silly). */
function EyesOpen() {
  const hi = 'bitfun-panda__eye-hi';
  const pu = 'bitfun-panda__pupil';
  return (
    <g className="bitfun-panda-head__eyes-open" aria-hidden>
      <Px x={7} y={12} w={4} h={4} className={hi} />
      <Px x={21} y={12} w={4} h={4} className={hi} />
      <Px x={9} y={14} w={1} h={1} className={pu} />
      <Px x={23} y={14} w={1} h={1} className={pu} />
    </g>
  );
}

/** Rest: soft “sleep” arcs — two short curves, not one stern bar. */
function FaceRest() {
  const hi = 'bitfun-panda__eye-hi';
  return (
    <g className="bitfun-panda-head__face bitfun-panda-head__face--rest">
      <Px x={7} y={14} w={4} h={1} className={hi} />
      <Px x={8} y={15} w={2} h={1} className={hi} />
      <Px x={21} y={14} w={4} h={1} className={hi} />
      <Px x={22} y={15} w={2} h={1} className={hi} />
      <Nose />
    </g>
  );
}

function FaceAnalyzing() {
  return (
    <g className="bitfun-panda-head__face bitfun-panda-head__face--analyze">
      <EyesOpen />
      <Nose />
      <g className="bitfun-panda-head__dots" aria-hidden>
        <Px x={2} y={6} w={2} h={2} className="bitfun-panda-head__dot" />
        <Px x={28} y={4} w={2} h={2} className="bitfun-panda-head__dot" />
        <Px x={15} y={1} w={2} h={2} className="bitfun-panda-head__dot" />
      </g>
    </g>
  );
}

function FaceWaiting() {
  return (
    <g className="bitfun-panda-head__face bitfun-panda-head__face--wait">
      <EyesOpen />
      <Nose />
      <g className="bitfun-panda-head__wait-pips" aria-hidden>
        <Px x={10} y={27} w={2} h={2} className="bitfun-panda-head__wait-pip" />
        <Px x={15} y={27} w={2} h={2} className="bitfun-panda-head__wait-pip" />
        <Px x={20} y={27} w={2} h={2} className="bitfun-panda-head__wait-pip" />
      </g>
    </g>
  );
}

function FaceWorking() {
  return (
    <g className="bitfun-panda-head__face bitfun-panda-head__face--work">
      <EyesOpen />
      <Nose />
      <g className="bitfun-panda-head__sweat" aria-hidden>
        <Px x={4} y={8} w={2} h={2} className="bitfun-panda-head__sweat-drop" />
        <Px x={26} y={7} w={2} h={2} className="bitfun-panda-head__sweat-drop" />
      </g>
      <g className="bitfun-panda-head__speed" aria-hidden>
        <Px x={0} y={10} w={2} h={2} className="bitfun-panda-head__speed-line" />
        <Px x={0} y={14} w={2} h={2} className="bitfun-panda-head__speed-line" />
        <Px x={0} y={18} w={2} h={2} className="bitfun-panda-head__speed-line" />
        <Px x={30} y={10} w={2} h={2} className="bitfun-panda-head__speed-line" />
        <Px x={30} y={14} w={2} h={2} className="bitfun-panda-head__speed-line" />
        <Px x={30} y={18} w={2} h={2} className="bitfun-panda-head__speed-line" />
      </g>
    </g>
  );
}

function Face({ mood }: { mood: ChatInputPetMood }) {
  switch (mood) {
    case 'rest':
      return <FaceRest />;
    case 'analyzing':
      return <FaceAnalyzing />;
    case 'waiting':
      return <FaceWaiting />;
    default:
      return <FaceWorking />;
  }
}

function MoodSvg({ mood }: { mood: ChatInputPetMood }) {
  const rootClass = `bitfun-panda-head bitfun-panda-head--${mood}`;

  return (
    <svg
      className={`bitfun-chat-input-pixel-pet__svg bitfun-chat-input-pixel-pet__svg--${mood}`}
      viewBox={`0 0 ${VIEW} ${VIEW}`}
      xmlns="http://www.w3.org/2000/svg"
      aria-hidden
    >
      <g className={rootClass}>
        <HeadHalo />
        <HeadDisk />
        <EarRims />
        <Ears />
        <EyePatches />
        <Face mood={mood} />
      </g>
    </svg>
  );
}

export const ChatInputPixelPet: React.FC<ChatInputPixelPetProps> = ({
  mood,
  className = '',
  layout = 'center',
}) => {
  const layoutMod =
    layout === 'stopRight' ? ' bitfun-chat-input-pixel-pet--layout-stop-right' : '';
  return (
    <div className={`bitfun-chat-input-pixel-pet${layoutMod} ${className}`.trim()} aria-hidden>
      {MOODS.map(m => (
        <div
          key={m}
          className="bitfun-chat-input-pixel-pet__layer"
          data-active={m === mood ? 'true' : 'false'}
        >
          <MoodSvg mood={m} />
        </div>
      ))}
    </div>
  );
};
