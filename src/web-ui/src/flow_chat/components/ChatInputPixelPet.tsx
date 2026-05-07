/**
 * BitFun chat-input mascot — sleeping panda (user-authored SVG geometry).
 *
 * Geometry: lifted verbatim from the user's hand-drawn panda.svg
 * (viewBox 320x204). The component layers:
 *
 *   Silhouette   — static panda body + face (always rendered, B/W only)
 *   FaceLayers   — per-mood decorative overlays (ZZZ, think pips, wait pips,
 *                  sweat). These ARE theme-inverted so they read on either
 *                  light or dark backgrounds.
 *
 * The panda silhouette stays black-on-white in BOTH themes. In dark theme
 * we add an outer light "halo" via CSS drop-shadow filter so the mascot
 * still pops on a dark chat input background, without inverting fur color.
 *
 * Mood signalling
 *   - rest:      gentle breathing + ZZZ + random in-place micro-actions
 *                every 4–8s (ear twitch, yawn, deep breath, body roll, paw
 *                wiggle) — never wanders out of place
 *   - analyzing: head tilts left/right, ears twitch, think pips ladder up
 *   - waiting:   head looks side-to-side, front paw taps, wait dots bob
 *   - working:   head micro-shakes, right ear tenses, sweat drop trickles
 */

import React, { useEffect, useRef, useState } from 'react';
import type { ChatInputPetMood } from '../utils/chatInputPetMood';
import type { AgentCompanionPetSelection } from '@/infrastructure/config/services/AIExperienceConfigService';
import { resolveAgentCompanionPetSrc } from '@/infrastructure/config/services/AgentCompanionPetService';
import './ChatInputPixelPet.scss';

export interface ChatInputPixelPetProps {
  mood: ChatInputPixelPetMood;
  className?: string;
  layout?: 'center' | 'stopRight';
  pet?: AgentCompanionPetSelection | null;
}

export type ChatInputPixelPetMood = ChatInputPetMood | 'hover' | 'dragging';

const VIEW_W = 320;
const VIEW_H = 204;

/* ---------- Static silhouette (verbatim from user's panda.svg) ---------- */

function Silhouette() {
  return (
    <g className="bitfun-panda-head__silhouette" aria-hidden>
      {/* Ears + back-hump bump */}
      <g className="bitfun-panda-head__ears">
        <g className="bitfun-panda-head__ear bitfun-panda-head__ear--left">
          <ellipse cx={50} cy={70} rx={29} ry={29} className="bitfun-panda__b" />
        </g>
        <g className="bitfun-panda-head__ear bitfun-panda-head__ear--right">
          <ellipse cx={181} cy={42} rx={28} ry={27} className="bitfun-panda__b" />
        </g>
      </g>
      <ellipse
        cx={259}
        cy={96}
        rx={22}
        ry={24}
        className="bitfun-panda__b"
        transform="rotate(-38 259 96)"
      />

      {/* Back body and rear leg */}
      <path
        className="bitfun-panda__b"
        d="M190 76 C216 68 242 78 262 100 C279 112 293 119 304 136 C315 152 311 165 292 168 C268 170 246 172 225 178 C196 185 168 181 156 166 C143 148 147 115 160 96 C167 86 177 80 190 76Z"
      />

      {/* Front body and left foreleg */}
      <g className="bitfun-panda-head__body bitfun-panda-head__body--front">
        <path
          className="bitfun-panda__b"
          d="M36 130 C33 91 60 45 113 36 C155 29 194 43 208 76 C223 111 206 154 168 171 C135 187 90 188 50 181 C25 176 13 165 15 151 C17 141 24 134 36 130Z"
        />
      </g>

      {/* White face */}
      <path
        className="bitfun-panda__face-mask"
        d="M55 132 C58 84 88 51 128 49 C169 47 200 68 201 103 C202 137 176 164 139 170 C103 175 69 160 58 139 C56 136 55 134 55 132Z"
      />

      {/* White shoulder */}
      <path
        className="bitfun-panda__face-mask"
        d="M207 84 C232 89 253 112 257 138 C260 153 252 164 237 166 C239 142 232 118 214 100 C210 96 207 91 207 84Z"
      />

      {/* Dark face-mask patches (eye patches) */}
      <path
        className="bitfun-panda__b"
        d="M69 139 C74 122 88 111 103 108 C112 107 116 113 112 121 C106 132 96 141 92 154 C90 162 80 164 73 158 C67 153 66 146 69 139Z"
      />
      <path
        className="bitfun-panda__b"
        d="M137 103 C150 98 171 103 181 116 C188 125 184 136 173 138 C160 140 145 134 137 123 C130 114 130 106 137 103Z"
      />

      {/* Dark front paws and chin outline (split into LEFT/RIGHT for paw taps) */}
      <g className="bitfun-panda-head__paw bitfun-panda-head__paw--front">
        <path
          className="bitfun-panda__b"
          d="M22 142 C39 129 72 130 97 142 C111 149 113 162 102 171 C87 184 49 181 27 170 C11 162 10 151 22 142Z"
        />
      </g>
      <g className="bitfun-panda-head__paw bitfun-panda-head__paw--rear">
        <path
          className="bitfun-panda__b"
          d="M151 144 C171 133 209 133 232 145 C247 153 246 165 231 173 C209 184 169 184 151 173 C138 165 138 152 151 144Z"
        />
      </g>

      {/* White muzzle (animated for yawn) */}
      <g className="bitfun-panda-head__muzzle">
        <path
          className="bitfun-panda__face-mask"
          d="M96 151 C104 138 122 136 137 140 C154 145 161 158 151 169 C142 178 115 179 103 171 C95 166 92 158 96 151Z"
        />
      </g>

      {/* Nose / mouth */}
      <ellipse
        cx={130}
        cy={153}
        rx={18}
        ry={8}
        className="bitfun-panda__b"
        transform="rotate(-8 130 153)"
      />
    </g>
  );
}

/* ---------- Mood overlays ---------- */

function FaceRest() {
  return (
    <g className="bitfun-panda-head__face bitfun-panda-head__face--rest">
      <g className="bitfun-panda-head__zzz" aria-hidden>
        <text
          x={215}
          y={75}
          className="bitfun-panda-head__zzz-glyph bitfun-panda-head__zzz-glyph--a"
        >
          z
        </text>
        <text
          x={245}
          y={45}
          className="bitfun-panda-head__zzz-glyph bitfun-panda-head__zzz-glyph--b"
        >
          z
        </text>
        <text
          x={278}
          y={18}
          className="bitfun-panda-head__zzz-glyph bitfun-panda-head__zzz-glyph--c"
        >
          Z
        </text>
      </g>
    </g>
  );
}

function FaceAnalyzing() {
  return (
    <g className="bitfun-panda-head__face bitfun-panda-head__face--analyze">
      <g className="bitfun-panda-head__think" aria-hidden>
        <circle cx={222} cy={72} r={4.5} className="bitfun-panda__b bitfun-panda-head__think-pip" />
        <circle cx={250} cy={48} r={6} className="bitfun-panda__b bitfun-panda-head__think-pip" />
        <circle cx={282} cy={20} r={8} className="bitfun-panda__b bitfun-panda-head__think-pip" />
      </g>
    </g>
  );
}

function FaceWaiting() {
  return (
    <g className="bitfun-panda-head__face bitfun-panda-head__face--wait">
      <g className="bitfun-panda-head__wait-pips" aria-hidden>
        <circle cx={228} cy={50} r={5} className="bitfun-panda-head__wait-pip" />
        <circle cx={252} cy={50} r={5} className="bitfun-panda-head__wait-pip" />
        <circle cx={276} cy={50} r={5} className="bitfun-panda-head__wait-pip" />
      </g>
    </g>
  );
}

function FaceWorking() {
  return (
    <g className="bitfun-panda-head__face bitfun-panda-head__face--work">
      {/* Sweat drop trickling down from forehead — classic "trying hard" cue. */}
      <g className="bitfun-panda-head__sweat" aria-hidden>
        <path
          d="M210 50 C204 60 204 72 210 76 C216 72 216 60 210 50 Z"
          className="bitfun-panda-head__sweat-drop"
        />
      </g>
    </g>
  );
}

function FaceHover() {
  return (
    <g className="bitfun-panda-head__face bitfun-panda-head__face--hover">
      <g className="bitfun-panda-head__sparkles" aria-hidden>
        <path d="M226 46 L232 58 L244 64 L232 70 L226 82 L220 70 L208 64 L220 58 Z" className="bitfun-panda-head__sparkle bitfun-panda-head__sparkle--a" />
        <path d="M270 20 L274 28 L282 32 L274 36 L270 44 L266 36 L258 32 L266 28 Z" className="bitfun-panda-head__sparkle bitfun-panda-head__sparkle--b" />
      </g>
    </g>
  );
}

function FaceDragging() {
  return (
    <g className="bitfun-panda-head__face bitfun-panda-head__face--drag">
      <g className="bitfun-panda-head__drag-lines" aria-hidden>
        <path d="M226 48 C244 40 262 40 282 48" className="bitfun-panda-head__drag-line bitfun-panda-head__drag-line--a" />
        <path d="M230 72 C248 64 268 65 286 75" className="bitfun-panda-head__drag-line bitfun-panda-head__drag-line--b" />
      </g>
    </g>
  );
}

const FACE_ORDER: ChatInputPixelPetMood[] = ['rest', 'analyzing', 'waiting', 'working', 'hover', 'dragging'];

function FaceFor(mood: ChatInputPixelPetMood) {
  switch (mood) {
    case 'rest':
      return <FaceRest />;
    case 'analyzing':
      return <FaceAnalyzing />;
    case 'waiting':
      return <FaceWaiting />;
    case 'hover':
      return <FaceHover />;
    case 'dragging':
      return <FaceDragging />;
    default:
      return <FaceWorking />;
  }
}

/* ---------- Idle micro-actions (rest mood only) ----------
 *
 * Every 4–8s while the mascot is resting, fire ONE local action that lasts
 * <2s and never moves the panda out of its slot. Rotation/translation is
 * applied to specific sub-elements via SCSS class selectors. */

const IDLE_ACTIONS = [
  'ear-twitch-left',
  'ear-twitch-right',
  'yawn',
  'deep-breath',
  'body-roll',
  'paw-wiggle',
] as const;

type IdleAction = (typeof IDLE_ACTIONS)[number];

const ACTION_DURATION_MS: Record<IdleAction, number> = {
  'ear-twitch-left': 700,
  'ear-twitch-right': 700,
  yawn: 1300,
  'deep-breath': 1800,
  'body-roll': 1500,
  'paw-wiggle': 900,
};

function pickIdleAction(prev: IdleAction | null): IdleAction {
  let pick = IDLE_ACTIONS[Math.floor(Math.random() * IDLE_ACTIONS.length)];
  if (prev && pick === prev) {
    pick = IDLE_ACTIONS[(IDLE_ACTIONS.indexOf(pick) + 1) % IDLE_ACTIONS.length];
  }
  return pick;
}

export const ChatInputPixelPet: React.FC<ChatInputPixelPetProps> = ({
  mood,
  className = '',
  layout = 'center',
  pet = null,
}) => {
  const layoutMod =
    layout === 'stopRight' ? ' bitfun-chat-input-pixel-pet--layout-stop-right' : '';

  const [petSrc, setPetSrc] = useState<string | null>(null);
  useEffect(() => {
    if (!pet) { setPetSrc(null); return; }
    let cancelled = false;
    void resolveAgentCompanionPetSrc(pet).then(src => {
      if (!cancelled) setPetSrc(src || null);
    });
    return () => { cancelled = true; };
  }, [pet]);

  const [transitioning, setTransitioning] = useState(false);
  const prevMoodRef = useRef<ChatInputPixelPetMood>(mood);
  useEffect(() => {
    if (prevMoodRef.current === mood) return;
    prevMoodRef.current = mood;
    setTransitioning(true);
    const t = window.setTimeout(() => setTransitioning(false), 320);
    return () => window.clearTimeout(t);
  }, [mood]);

  const [idleAction, setIdleAction] = useState<IdleAction | null>(null);
  const idleActionRef = useRef<IdleAction | null>(null);
  useEffect(() => {
    if (mood !== 'rest') {
      setIdleAction(null);
      idleActionRef.current = null;
      return;
    }

    let stopped = false;
    let nextTimer = 0;
    let clearTimer = 0;

    const schedule = () => {
      if (stopped) return;
      // First action comes quickly so the user notices the pet is alive.
      const wait = idleActionRef.current === null ? 1600 : 4000 + Math.random() * 4000;
      nextTimer = window.setTimeout(() => {
        if (stopped) return;
        const next = pickIdleAction(idleActionRef.current);
        idleActionRef.current = next;
        setIdleAction(next);
        clearTimer = window.setTimeout(() => {
          if (stopped) return;
          setIdleAction(null);
          schedule();
        }, ACTION_DURATION_MS[next] + 60);
      }, wait);
    };

    schedule();
    return () => {
      stopped = true;
      window.clearTimeout(nextTimer);
      window.clearTimeout(clearTimer);
    };
  }, [mood]);

  const stageClasses = [
    'bitfun-chat-input-pixel-pet__stage',
    `bitfun-chat-input-pixel-pet__stage--${mood}`,
    transitioning ? 'bitfun-chat-input-pixel-pet__stage--transition' : '',
    idleAction ? `bitfun-chat-input-pixel-pet__stage--idle-${idleAction}` : '',
  ]
    .filter(Boolean)
    .join(' ');

  if (pet && petSrc) {
    const rowByMood: Record<ChatInputPixelPetMood, number> = {
      rest: 0,
      hover: 1,
      dragging: 2,
      analyzing: 8,
      waiting: 6,
      working: 7,
    };
    return (
      <div className={`bitfun-chat-input-pixel-pet${layoutMod} ${className}`.trim()} aria-hidden>
        <div
          className={`bitfun-chat-input-pixel-pet__petdex bitfun-chat-input-pixel-pet__petdex--${mood}`}
          style={{
            '--bitfun-petdex-src': `url("${petSrc}")`,
            '--bitfun-petdex-row': rowByMood[mood],
          } as React.CSSProperties}
        />
      </div>
    );
  }

  return (
    <div className={`bitfun-chat-input-pixel-pet${layoutMod} ${className}`.trim()} aria-hidden>
      <div className={stageClasses}>
        <svg
          className={`bitfun-chat-input-pixel-pet__svg bitfun-chat-input-pixel-pet__svg--${mood}`}
          viewBox={`0 0 ${VIEW_W} ${VIEW_H}`}
          xmlns="http://www.w3.org/2000/svg"
          aria-hidden
        >
          {/* Outline filter — only applied in dark theme via SCSS. Builds a
             single continuous light contour around the union of all dark
             shapes by dilating the silhouette's alpha and flooding the
             expanded area with the outline color, then compositing the
             original graphic on top. */}
          <defs>
            <filter
              id="bitfun-panda-outline"
              x="-15%"
              y="-15%"
              width="130%"
              height="130%"
            >
              <feMorphology in="SourceAlpha" operator="dilate" radius="6" result="OUT" />
              <feFlood floodColor="#fafaf9" floodOpacity="0.25" />
              <feComposite in2="OUT" operator="in" result="OUT_FILLED" />
              <feMerge>
                <feMergeNode in="OUT_FILLED" />
                <feMergeNode in="SourceGraphic" />
              </feMerge>
            </filter>
          </defs>
          <g className={`bitfun-panda-head bitfun-panda-head--${mood}`}>
            <Silhouette />
            <g className="bitfun-panda-head__faces">
              {FACE_ORDER.map(m => (
                <g
                  key={m}
                  className="bitfun-panda-head__face-layer"
                  data-active={m === mood ? 'true' : 'false'}
                >
                  {FaceFor(m)}
                </g>
              ))}
            </g>
          </g>
        </svg>
      </div>
    </div>
  );
};
