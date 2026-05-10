**中文** | [English](README.md)

# 字体说明

本目录提供 Web UI 的字体资源与配置说明。

## 目录结构

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

- 用途：代码编辑器与终端
- 来源：https://github.com/tonsky/FiraCode
- 许可证：SIL Open Font License 1.1 (OFL-1.1)
- 本地许可证：`FiraCode/LICENSE.txt`

| 字重 | 文件 | 用途 |
|------|------|------|
| 400 | FiraCode-Regular.woff2 | 常规代码 |
| 500 | FiraCode-Medium.woff2 | 强调 |
| 600 | FiraCode-SemiBold.woff2 | 关键字 |
| 300-700 | FiraCode-VF.woff2 | 可变字体 |

## Noto Sans SC

- 用途：UI 界面（中英文显示）
- 来源：https://fonts.google.com/noto/specimen/Noto+Sans+SC
- 许可证：SIL Open Font License 1.1 (OFL-1.1)
- 本地许可证：`Noto_Sans_SC/OFL.txt`

| 字重 | 文件 | 用途 |
|------|------|------|
| 400 | NotoSansSC-Regular.ttf | 正文 |
| 500 | NotoSansSC-Medium.ttf | 标题/强调 |
| 600 | NotoSansSC-SemiBold.ttf | 重要标题 |

仅打包 `fonts.css` 实际引用的静态字重。

## 字体配置

代码字体：
```scss
'Fira Code', 'Noto Sans SC', Consolas, 'Courier New', monospace
```
中文使用 `Noto Sans SC` 字体名，由 `fonts.css` 提供。

界面字体：
```scss
'Noto Sans SC', 'PingFang SC', 'Microsoft YaHei', -apple-system, BlinkMacSystemFont, 'Segoe UI', 'SF Pro Display', Roboto, sans-serif
```

## 注意事项

- 使用 `font-display: swap` 提升显示速度
- 编辑器已启用连字（`fontLigatures: true`）
- 字体缺失时会降级到系统字体
