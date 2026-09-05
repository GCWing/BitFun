# OpenBitFun application brand assets

`source/openbitfun-mark.svg` is the transparent vector master for the current
fine-line Logo used by startup and in-product brand surfaces. Its fifteen rounded
hexagonal contours match the About dialog's static geometry. Motion and moving
highlights are intentionally absent from static files. The SVG uses
`currentColor`, with a light default for dark backgrounds.

The generator also maintains the existing transparent PNG paths:

- `openbitfun-mark-dark.png` is the dark mark for light surfaces.
- `openbitfun-mark-light.png` is the light mark for dark surfaces.

`source/openbitfun-app-mark.png` preserves the originally submitted silver
hexagonal application mark. Application, window, taskbar, Dock, tray, browser,
installer, and mobile launcher icons use this artwork on the black rounded-square
background with transparent corners. It is intentionally independent from the
current startup Logo, so regenerating either family cannot replace the other.

`exports/` contains the SVG, ICO, ICNS, and PNGs at 16, 24, 32, 48, 64, 96,
128, 192, 256, 512, 1024, and 2048 px. Each PNG size includes a dark transparent
mark, a light transparent mark, and the application icon. Fine-line mark exports
are rendered directly from the SVG with size-specific optical treatments at
small sizes. Application icons are resized from the preserved application artwork.
Windows ICO frames, Linux icons, and macOS PNG representations use the matching
size-specific application exports. Tauri encodes the legacy 16/32 px ICNS
representations from the same artwork.

Browser entry points select explicit 16/32 px favicons. The system tray reuses
the configured application/window icon on every desktop platform.

Run `pnpm run generate-brand-assets` after changing either source master. The
generator is the single owner of the derived desktop, web, installer, Android,
iOS, and HarmonyOS files.

Verify generated dimensions, small-size rim contrast, favicon references, and
icon containers with `node --test scripts/generate-brand-assets.test.mjs`.
