# OpenBitFun application brand assets

`source/openbitfun-mark.svg` is the transparent vector master for the fine-line
Logo. Its fifteen rounded hexagonal contours match the About dialog's static
geometry. Motion and moving highlights are intentionally absent from icon files.
The SVG uses `currentColor`, with a light default for dark backgrounds.
The outer contour is opaque and heavier than the interior filaments so the
white rim remains legible when the master is reduced by an application host.

The generator also maintains the existing transparent PNG paths:

- `openbitfun-mark-dark.png` is the dark mark for light surfaces.
- `openbitfun-mark-light.png` is the light mark for dark surfaces.

The application icon uses the light mark on the existing black rounded-square
background, with transparent corners. The 1024 px master is rendered directly
from the vector before generating Windows ICO and macOS ICNS containers.

`exports/` contains the SVG, ICO, ICNS, and PNGs at 16, 24, 32, 48, 64, 96,
128, 192, 256, 512, 1024, and 2048 px. Each PNG size includes a dark transparent
mark, a light transparent mark, and the application icon. PNG exports are
rendered directly from the SVG rather than enlarged from a small bitmap.
At 16–24 px, 32–48 px, and 64–96 px, PNGs use 3, 5, and 7 contours respectively
with a minimum 1.35 px opaque outer rim, a 0.85 px inner contour, and 0.65 px
interior filaments. Larger PNGs and the SVG retain all 15 contours.
Windows ICO frames and Linux icons use the same size-specific renders; macOS
PNG representations do too. Tauri encodes the legacy 16/32 px ICNS representations
from their corresponding optical renders.

Browser entry points select explicit 16/32 px favicons. The macOS menu bar uses
`src/apps/desktop/icons/openbitfun-tray-template.png`, a transparent 32 px template
drawn for a 16 pt display, so the system can tint it for light and dark menus.
Windows and Linux trays use the 32 px application icon.

Run `pnpm run generate-brand-assets` after changing the SVG master. The
generator is the single owner of the derived desktop, web, installer, Android,
iOS, and HarmonyOS files.

Verify generated dimensions, small-size rim contrast, favicon references, and
icon containers with `node --test scripts/generate-brand-assets.test.mjs`.
