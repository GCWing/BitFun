# bitfun-transport

**描述**: BitFun 传输层，跨平台通信适配器。

**包名**: `bitfun-transport` | lib: `bitfun_transport`

## 核心模块

| 模块 | 说明 |
|------|------|
| `traits` | 传输适配器接口定义 |
| `emitter` | 事件发射器实现 |
| `adapters` | 具体适配器实现（如 Tauri） |

## 关键类型/功能

- `TransportAdapter` trait — 传输适配器抽象接口
- `TransportEmitter` — 事件发射器，封装事件发送
- `TauriTransportAdapter` (feature=tauri-adapter) — Tauri 平台传输实现

## 一句话总结

跨平台事件传输抽象层，通过 TransportAdapter trait 统一应用内事件传递接口。
