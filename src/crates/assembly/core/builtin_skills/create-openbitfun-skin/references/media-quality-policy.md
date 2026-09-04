# Media quality policy

Preserve the user's source file. Generated package assets are derivatives selected to satisfy OpenBitFun package limits and runtime compatibility.

## Default video policy

Use `auto` unless the user explicitly requests another policy.

`auto` selects the highest quality that satisfies the 64 MiB background-video limit:

1. For sources at or below 12 million pixel-seconds after the 60-second cap, try VP9 codec-lossless encoding first.
2. For larger sources, or when the lossless attempt exceeds the limit, start at CRF 20.
3. When an attempt exceeds 64 MiB, increase CRF according to the measured size ratio and retry.
4. Stop at the first valid output and record every attempt plus the selected result.

The host transform still caps duration at 60 seconds, bounds landscape output to 3840x2160 and portrait output to 2160x3840, normalizes playback to 30 FPS, converts to `yuv420p`, removes audio, and strips metadata. It converts non-square source pixels to `SAR 1:1` while preserving the source display aspect ratio, so browser `videoWidth` and `videoHeight` remain inside the host limits.

Treat the browser display dimensions as authoritative. FFprobe's coded `width` and `height` may look valid while a non-1:1 sample aspect ratio expands the displayed video beyond 4,096 pixels per side or 9 million pixels. Generated videos and extracted still frames must both normalize the sample aspect ratio.

`codecLossless: true` means VP9 does not add quantization loss after that transform. It does not mean the packaged derivative is byte-for-byte or pixel-for-pixel identical to the original source.

Available policies:

- `auto`: adaptive highest quality; default.
- `lossless`: require codec-lossless VP9 and fail when it exceeds 64 MiB.
- `high`: use CRF 20 and fail when it exceeds the limit.
- `balanced`: use CRF 32 and fail when it exceeds the limit.
- explicit `--video-crf`: override the policy for reproducibility or manual tuning.

Do not silently fall back from an explicitly requested fixed policy. Report the size failure so the user can choose a different policy.

## Default static policy

Use `auto` for generated WebP assets:

1. Try lossless WebP.
2. Keep it when it satisfies the asset limit.
3. Otherwise try lossy qualities 95, 92, 88, 84, 80, 75, and 70.
4. Stop at the highest quality that satisfies the limit.

Ordinary images are limited to 16 MiB. The package preview is limited to 4 MiB. Explicit lossless or quality modes fail instead of silently changing the user's requested policy.

Record the selected mode, quality, file size, and prior attempts in the build record. Runtime visual inspection remains required even when lossless encoding succeeds because crop, tint, blur, scaling, and moving backgrounds affect readability independently of codec quality.
