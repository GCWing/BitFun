# Cinematic palette contract

Use a semantic palette to generate materials and renderer settings. Palette
files use this shape:

```json
{
  "schema": "openbitfun.appearance.cinematic-palette",
  "schemaVersion": 1,
  "id": "cold-cinematic",
  "colors": {
    "background": "#08111f",
    "surface": "#102235",
    "surfaceElevated": "#182b3c",
    "text": "#edf7fa",
    "textSecondary": "#c5d6dc",
    "textMuted": "#8aa2ab",
    "textDisabled": "#60757c",
    "accent": "#52e5f5",
    "accentStrong": "#2fc4d7",
    "accentContrast": "#041416",
    "info": "#78a9ff",
    "success": "#7dcc9c",
    "warning": "#e2b96b",
    "danger": "#e47b74"
  }
}
```

Every color is required and must be a six-digit hex value. `background`,
`surface`, and `surfaceElevated` drive translucent materials. `accent` drives
focus, borders, and primary highlights. Semantic status colors remain distinct
from the primary accent.

Prefer colors sampled from the source artwork. Preserve readable contrast and
keep Monaco and xterm backgrounds opaque. The scaffold derives alpha values;
do not put alpha channels in the palette.
