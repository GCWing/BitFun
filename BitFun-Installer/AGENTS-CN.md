**中文** | [English](AGENTS.md)

# AGENTS-CN.md

## 适用范围

本文件适用于 `BitFun-Installer`。仓库级规则请看顶层 `AGENTS.md`。

## 这里最重要的内容

`BitFun-Installer` 是独立的 Tauri + React 应用，不属于主 Cargo workspace。

模块 README 明确提到的重要区域：

- `src-tauri/src/installer/commands.rs`：Tauri IPC 与卸载执行
- `src-tauri/src/installer/registry.rs`：Windows 注册表集成
- `src-tauri/src/installer/shortcut.rs`：快捷方式创建
- `src-tauri/src/installer/extract.rs`：压缩包解压
- `src/hooks/useInstaller.ts`：前端安装流程状态
- `src/i18n/`：安装器专属文案；locale 元数据由
  `src/shared/i18n/contract/locales.json` 生成

安装流程：

```text
Language Select → Options → Progress → Model Setup → Theme Setup
```

## 命令

这些是命令参考，不是默认预检清单。PR 范围请以下方 Verification 为准。

```bash
pnpm --dir BitFun-Installer run installer:dev
pnpm --dir BitFun-Installer run tauri:dev
pnpm --dir BitFun-Installer run type-check
pnpm --dir BitFun-Installer run build            # React build / 复现 CI
pnpm --dir BitFun-Installer run installer:build  # 仅打包场景
```

## 验证

按触及范围选择最小检查：

```bash
pnpm run i18n:audit                                                   # 仅资源类 i18n
pnpm run i18n:generate && pnpm run i18n:contract:test && pnpm run i18n:audit
pnpm --dir BitFun-Installer run type-check                            # 前端 i18n/runtime
cargo check --manifest-path BitFun-Installer/src-tauri/Cargo.toml      # Tauri/Rust 改动
```

只有修改打包、payload、native bundling、安装/卸载流程、注册表、快捷方式或解压逻辑时，才运行完整安装器构建：

```bash
pnpm --dir BitFun-Installer run type-check && pnpm --dir BitFun-Installer run installer:build
```

如果修改了卸载流程，还需要验证 `BitFun-Installer/README.md` 中描述的卸载模式入口。
