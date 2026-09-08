# BitFun Model Router 客户端

本目录包含固定的 Router 系统提示词和可移植的 BitFun 启动脚本。启动脚本负责让
BitFun 连接已经运行的 OpenAI-compatible Router 服务，不会在客户端或评测机器上
启动 vLLM。

## 目录内容

- `router_system_prompt.txt`：BitFun 在每个主 Agent round 开始前发送给 Router 的
  固定系统提示词。
- `run_bitfun_with_router.sh`：配置 Router 客户端环境变量，然后完整启动 BitFun；不传
  参数时进入原有交互模式，传入参数时则原样转发。


## 编译 BitFun

在 BitFun 仓库根目录执行：

```bash
cargo build --locked -p openbitfun-cli --bin openbitfun
```

启动脚本默认使用 `target/debug/openbitfun`。如果使用 release 构建或复制到其他位置的
二进制文件，请通过 `BITFUN_BIN` 指定路径。

## 连接远程 Router

打开 `run_bitfun_with_router.sh`，直接替换顶部的 Router API URL 和 API key：

```bash
export ROUTER_API_URL='http://116.204.115.119:8001/v1'
export ROUTER_API_KEY='<请替换为实际API key>'
```

然后进入需要 BitFun 操作的目标代码仓库，直接启动完整的交互式 BitFun：

```bash
/path/to/BitFun/deploy/model-router/run_bitfun_with_router.sh
```

脚本没有改变 BitFun 的运行模式，只是在启动前注入 Router 配置。进入交互界面后可以
像原版 BitFun 一样连续对话，每个主 Agent round 都会自动调用 Router。

需要执行单个非交互任务时，才传入 BitFun CLI 的 `exec` 参数：

```bash
/path/to/BitFun/deploy/model-router/run_bitfun_with_router.sh \
  exec '真实任务描述'
```

下列参数均属于 BitFun CLI，而不是 Router 参数，并且都由用户按需添加：

- `-v`：输出更详细的 BitFun 执行日志，主要用于调试；批量评测时可以去掉。
- `exec`：以非交互任务模式运行 BitFun。
- `--output-format json`：可选；仅当外部程序明确需要解析 BitFun 的 JSON 输出时使用。

Router 的单行 JSON 输出由 BitFun 内部请求和 `router_system_prompt.txt` 控制，与上述
三个 CLI 参数无关。

默认 Router 客户端配置如下：

```text
Router 模型：       router-best
最近轨迹轮数：      3
Simple 阈值：       0.7
请求超时：          10000 ms
```

可以分别通过 `ROUTER_MODEL`、`ROUTER_RECENT_ROUNDS`、
`ROUTER_SIMPLE_THRESHOLD` 和 `ROUTER_TIMEOUT_MS` 覆盖这些默认值。

BitFun 会在每个主 Agent round 开始前，自动使用原始任务、可用的历史摘要和最近完成的
round 组装 Router 输入。评测程序只需在启动 BitFun 时传入真实任务描述，不需要自行
逐轮构造 Router 输入。
