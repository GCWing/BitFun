[中文](README.zh-CN.md) | **English**

# Fonts

This directory contains font assets and configuration for the web UI.

## Structure

```
src/web-ui/public/fonts/
├── fonts.css
├── README.md
├── FiraCode/
│   ├── FiraCode-Regular.woff2
│   ├── FiraCode-Medium.woff2
│   ├── FiraCode-SemiBold.woff2
│   └── FiraCode-VF.woff2
└── Noto_Sans_SC/
    └── static/
        ├── NotoSansSC-Regular.ttf
        ├── NotoSansSC-Medium.ttf
        └── NotoSansSC-SemiBold.ttf
```

## Fira Code

- Use: code editor and terminal
- Source: https://github.com/tonsky/FiraCode
- License: SIL Open Font License 1.1 (OFL-1.1)
- Local license: `FiraCode/LICENSE.txt`

| Weight | File | Usage |
|------|------|------|
| 400 | FiraCode-Regular.woff2 | Regular code |
| 500 | FiraCode-Medium.woff2 | Emphasis |
| 600 | FiraCode-SemiBold.woff2 | Keywords |
| 300-700 | FiraCode-VF.woff2 | Variable font |

## Noto Sans SC

- Use: UI text (Chinese and English)
- Source: https://fonts.google.com/noto/specimen/Noto+Sans+SC
- License: SIL Open Font License 1.1 (OFL-1.1)
- Local license: `Noto_Sans_SC/OFL.txt`

| Weight | File | Usage |
|------|------|------|
| 400 | NotoSansSC-Regular.ttf | Body |
| 500 | NotoSansSC-Medium.ttf | Heading/Emphasis |
| 600 | NotoSansSC-SemiBold.ttf | Strong heading |

Only the static weights referenced by `fonts.css` are bundled.

## Font Stack

Mono:
```scss
'Fira Code', 'Noto Sans SC', Consolas, 'Courier New', monospace
```
Chinese text uses `Noto Sans SC` as defined in `fonts.css`.

Sans:
```scss
'Noto Sans SC', 'PingFang SC', 'Microsoft YaHei', -apple-system, BlinkMacSystemFont, 'Segoe UI', 'SF Pro Display', Roboto, sans-serif
```

## Notes

- `font-display: swap` is enabled
- Editor ligatures are enabled (`fontLigatures: true`)
- Missing fonts fall back to system fonts
