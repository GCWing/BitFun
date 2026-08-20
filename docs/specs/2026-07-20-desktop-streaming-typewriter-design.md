# Desktop Streaming Typewriter Smoothness

**Date:** 2026-07-20  
**Status:** Approved for implementation (user authorized direct design + implement)

## Problem

bitfun-desktop session streaming text feels stepped / bursty compared with a smooth typewriter feel (mobile-web cited as a reference, not a target to copy).

## Root Cause

1. `EventBatcher` flushes all events on a fixed **100ms** cadence, so text arrives in stair steps.
2. `useTypewriter` uses `setInterval(50ms)` and **forces catch-up within 800ms**, so large batches raise `chars/tick` sharply and produce visible jumps.
3. Full Markdown re-parse on each display update makes oversized ticks more expensive and more visible.

## Design Goals

- Smooth, even visual velocity (not “true realtime at all costs”).
- Avoid feast/famine catch-up bursts.
- Keep tool-event batching protective (do not spam UI with ParamsPartial/Progress).
- Stay inside `src/web-ui` streaming path; no Rust / mobile-web changes.

## Solution

### 1. Adaptive rAF typewriter (`useTypewriter`)

Replace fixed interval + 800ms catch-up with:

- `requestAnimationFrame` loop
- Fractional character accumulator (sub-frame smoothness)
- Soft acceleration from backlog depth
- Hard cap on characters revealed per paint
- Minimum paint interval (~32ms) so Markdown is not forced to 60Hz full re-parse
- When the model stream ends (`animate === false`) while characters remain,
  keep revealing with a finish-speed boost — never snap the remainder
- History / never-animated mounts still start at full text

Tunable constants (exported for tests):

| Constant | Intent |
|---|---|
| Base ~90 chars/s | Comfortable live reveal |
| Soft accel from backlog | Catch up without dumping |
| Live max ~720 chars/s / 18 chars/paint | Fast but stepped while streaming |
| Finish max ~2400 chars/s / 64 chars/paint | Fastest stepped drain after model ends |
| Min paint ~16ms live / ~8ms finish | Balance smoothness vs Markdown cost |

Footer / round chrome wait for typewriter reveal completion via
`TypewriterRevealGate`, so copy/export actions do not appear (and reflow)
while characters are still being revealed.

`ModelRoundItem` must not replay CSS `fadeIn` when flipping
`--streaming` → `--complete` after typewriter drain (that reset opacity to 0
and looked like a full chat refresh). Enter animation is opt-in via
`--enter` only for freshly mounted non-streaming rounds.

### 2. Dual-latency `EventBatcher`

- Text chunk keys: **maxLatencyMs = 32** (steadier inflow for the typewriter)
- Tool / default events: **maxLatencyMs = 100** (unchanged protection)
- If a more urgent event arrives while a flush is already scheduled, **reschedule** to the earlier deadline

`handleTextChunk` passes the text latency when calling `eventBatcher.add`.

### 3. Out of scope (follow-ups)

- Incremental / frozen-block Markdown parsing
- VirtualMessageList observer frequency changes
- mobile-web typewriter changes

## Verification

- Unit tests for reveal-step math and EventBatcher dual-latency / reschedule
- `pnpm run type-check:web`
- Focused vitest for touched modules
- Manual: stream a long mixed CJK/Latin reply in desktop and confirm even pacing without stair-step bursts
