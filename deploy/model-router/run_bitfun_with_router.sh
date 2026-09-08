#!/usr/bin/env bash
set -euo pipefail

script_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
repo_root=$(cd -- "$script_dir/../.." && pwd)

# 用户配置：替换为实际的 Router 地址和 API key。
export ROUTER_API_URL='http://116.204.115.119:8001/v1'
export ROUTER_API_KEY='<请替换为实际API key>'

# 默认执行本仓库已编译的 BitFun；也可通过 BITFUN_BIN 指定其他二进制。
bitfun_bin=${BITFUN_BIN:-$repo_root/target/debug/openbitfun}

router_endpoint=${ROUTER_API_URL%/}
if [[ "$router_endpoint" != */chat/completions ]]; then
  router_endpoint=$router_endpoint/chat/completions
fi

export OPENBITFUN_ROUND_ROUTER_URL=$router_endpoint
export OPENBITFUN_ROUND_ROUTER_MODEL=${ROUTER_MODEL:-router-best}
export OPENBITFUN_ROUND_ROUTER_API_KEY=$ROUTER_API_KEY
export OPENBITFUN_ROUND_ROUTER_PROMPT=${ROUTER_SYSTEM_PROMPT:-$script_dir/router_system_prompt.txt}
export OPENBITFUN_ROUND_ROUTER_TIMEOUT_MS=${ROUTER_TIMEOUT_MS:-10000}
export OPENBITFUN_ROUND_ROUTER_RECENT_ROUNDS=${ROUTER_RECENT_ROUNDS:-3}
export OPENBITFUN_ROUND_ROUTER_SIMPLE_THRESHOLD=${ROUTER_SIMPLE_THRESHOLD:-0.7}

if [[ -n ${ROUTER_TRACE_PATH:-} ]]; then
  export OPENBITFUN_ROUND_ROUTER_TRACE=$ROUTER_TRACE_PATH
fi

# 不添加参数时保持 BitFun 原本的交互模式；传入 exec 等参数时则原样转发。
exec "$bitfun_bin" "$@"
