# OpenBitFun Model Router 客户端

本目录包含固定的 Router 系统提示词和可移植的 OpenBitFun 启动脚本。启动脚本负责让
OpenBitFun 连接已经运行的 OpenAI-compatible Router 服务，不会在客户端或评测机器上
启动 vLLM。

## 目录内容

- `router_system_prompt.txt`：OpenBitFun 在每个主 Agent round 开始前发送给 Router 的
  固定系统提示词。
- `run_openbitfun_with_router.sh`：配置 Router 客户端环境变量，然后完整启动 OpenBitFun；不传
  参数时进入原有交互模式，传入参数时则原样转发。

## 编译 OpenBitFun

在 OpenBitFun 仓库根目录执行：

```bash
cargo build --locked -p openbitfun-cli --bin openbitfun
```

启动脚本默认使用 `target/debug/openbitfun`。如果使用 release 构建或复制到其他位置的
二进制文件，请通过 `OPENBITFUN_BIN` 指定路径。

## 连接远程 Router

通过环境变量传入 Router API URL；只有 Router 要求鉴权时才设置 API key：

```bash
export ROUTER_API_URL='http://<router-host>:<port>/v1'
# export ROUTER_API_KEY='<仅在需要鉴权时填写>'
```

然后进入需要 OpenBitFun 操作的目标代码仓库，直接启动完整的交互式 OpenBitFun：

```bash
/path/to/OpenBitFun/deploy/model-router/run_openbitfun_with_router.sh
```

脚本没有改变 OpenBitFun 的运行模式，只是在启动前注入 Router 配置。进入交互界面后可以
像原版 OpenBitFun 一样连续对话，每个主 Agent round 都会自动调用 Router。

需要执行单个非交互任务时，才传入 OpenBitFun CLI 的 `exec` 参数：

```bash
/path/to/OpenBitFun/deploy/model-router/run_openbitfun_with_router.sh \
  exec '真实任务描述'
```

下列参数均属于 OpenBitFun CLI，而不是 Router 参数，并且都由用户按需添加：

- `-v`：输出更详细的 OpenBitFun 执行日志，主要用于调试；批量评测时可以去掉。
- `exec`：以非交互任务模式运行 OpenBitFun。
- `--output-format json`：可选；仅当外部程序明确需要解析 OpenBitFun 的 JSON 输出时使用。

Router 的单行 JSON 输出由 OpenBitFun 内部请求和 `router_system_prompt.txt` 控制，与上述
三个 CLI 参数无关。

## Router 服务兼容要求

阈值路由除了标准的 `chat/completions` 内容外，还依赖 vLLM 提供的 `logprobs`、
`top_logprobs`、`return_token_ids` 和 `return_tokens_as_token_ids` 扩展字段。当前
`simple` / `non_simple` 的 token ID 与 `router-best` 使用的 tokenizer 绑定；更换
Router 模型或 tokenizer 后，需要先核对这两个 token ID。服务不返回完整置信度数据时，
OpenBitFun 会保守地选择主力模型。

默认 Router 客户端配置如下：

```text
Router 模型：       router-best
最近轨迹轮数：      3
最大输入字符数：    80000
Simple 阈值：       0.7
请求超时：          10000 ms
```

可以分别通过 `ROUTER_MODEL`、`ROUTER_RECENT_ROUNDS`、
`ROUTER_MAX_INPUT_CHARS`、`ROUTER_SIMPLE_THRESHOLD` 和 `ROUTER_TIMEOUT_MS`
覆盖这些默认值。

OpenBitFun 会在每个主 Agent round 开始前，自动使用原始任务、可用的历史摘要和最近完成的
round 组装 Router 输入。评测程序只需在启动 OpenBitFun 时传入真实任务描述，不需要自行
逐轮构造 Router 输入。

## 数据与安全

Router 请求可能包含任务原文、历史摘要、推理文本、工具参数和工具结果，因此只应连接受信任
的 Router 服务。启用 `ROUTER_TRACE_PATH` 后，本地 trace 也会记录完整 Router 输入和输出；
评测结束后请按数据敏感级别妥善保管或清理该文件。
