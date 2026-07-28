# Taiji 策略模板 — DualThrust 通道突破

> 参考：WonderTrader/wtpy `demos/Strategies/DualThrust.py`（MIT），Michael Chalek (1980s)

## 架构：策略即后端

```
┌─────────────────────────────────────────────────┐
│  工具层（开源 MIT）                               │
│  taiji-cli / Tauri desktop / Python SDK          │
│  → 负责：YAML 配置、数据管线、信号输出、报告/视频  │
├─────────────────────────────────────────────────┤
│  trait ComputeNode（接口契约，开源 MIT）           │
│  → 定义：id(), input_keys(), on_bar(), on_init() │
├─────────────────────────────────────────────────┤
│  你的策略 crate（闭源，编译进 binary）              │
│  → 实现：evaluate() 量价时空公式                   │
│  → 前端调用：register_node! 一行挂载               │
└─────────────────────────────────────────────────┘
```

工具和策略通过 `ComputeNode` trait 解耦。你只暴露 trait 实现，策略源码编译进 binary，不对外分发。换个人拿自己的策略，换掉策略 crate 重新编译即可。

## 三步上手

**1. 复制模板**
```bash
cp -r src/crates/taiji/taiji-strategy-template src/crates/taiji/my-strategy
```

**2. 改名字 + 实现策略**
- `Cargo.toml`: 改 `name`、`description`
- `src/lib.rs`: 结构体 `DualThrust` → 你的策略名
- `evaluate()`: 写入你的量价时空公式

**3. 注册到工具**
```rust
// taiji-cli/src/main.rs
register_node!(factory, "my_strategy", my_strategy::MyStrategy, "my_strategy_1");
```
编译后策略逻辑在 binary 内，源码不暴露。

## 策略配置（YAML）

```yaml
nodes:
  - id: dual_1
    type: dual_thrust
    config:
      lookback: 20
      k1: 0.7
      k2: 0.7
    subscribe: ["bars:1m"]
```

## 策略→教学→视频 全链路

策略输出 `signal:<strategy_id>` → 下游自动消费：

```
[Bars:1m] → [你的策略] → [ReportGen] → [VideoRender] → [多平台发布]
                ↓
           [AlertManager]
```

配置见 `examples/strategy-to-video-pipeline.yaml`
