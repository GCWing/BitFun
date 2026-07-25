# bitfun-webdriver

**描述**: 嵌入在 BitFun 桌面应用内的 WebDriver 服务器。

**包名**: `bitfun-webdriver` | lib: `bitfun_webdriver`

## 核心模块

| 模块 | 说明 |
|------|------|
| `server` | HTTP WebDriver 服务器 |
| `webdriver` | WebDriver 协议实现 |
| `executor` | 命令执行器 |
| `platform` | 平台特定实现 |
| `runtime` | 运行时事件监听 |

## 关键类型/功能

- `maybe_start()` — 按条件启动 WebDriver 服务器（debug/embedded 模式）
- `handle_bridge_result()` — 处理 Tauri 桥接结果
- `AppState` — 服务器共享状态
- 环境变量控制: `BITFUN_WEBDRIVER_PORT` / `BITFUN_WEBDRIVER_LABEL`

## 平台支持

| 平台 | 依赖 |
|------|------|
| macOS | `objc2-app-kit`, `objc2-web-kit` |
| Windows | `webview2-com` |
| Linux | `webkit2gtk`, `gtk` |

## 设计要点

- 仅在 debug 模式或 `embedded` feature 下启动
- 通过 Tauri 应用句柄 (`AppHandle`) 控制
- 支持通过 WebDriver 协议自动化浏览器操作

## 一句话总结

嵌入桌面应用的 WebDriver 服务器，支持通过标准 WebDriver 协议控制内嵌浏览器。
