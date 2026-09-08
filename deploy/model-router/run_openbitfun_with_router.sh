#!/usr/bin/env bash
set -euo pipefail

script_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
repo_root=$(cd -- "$script_dir/../.." && pwd)

: "${ROUTER_API_URL:?Set ROUTER_API_URL to the Router base URL or chat/completions endpoint}"

# 默认执行本仓库已编译的 OpenBitFun；也可通过 OPENBITFUN_BIN 指定其他二进制。
openbitfun_bin=${OPENBITFUN_BIN:-$repo_root/target/debug/openbitfun}

router_endpoint=${ROUTER_API_URL%/}
if [[ "$router_endpoint" != */chat/completions ]]; then
  router_endpoint=$router_endpoint/chat/completions
fi

export OPENBITFUN_ROUND_ROUTER_URL=$router_endpoint
export OPENBITFUN_ROUND_ROUTER_MODEL=${ROUTER_MODEL:-router-best}
export OPENBITFUN_ROUND_ROUTER_PROMPT=${ROUTER_SYSTEM_PROMPT:-$script_dir/router_system_prompt.txt}
export OPENBITFUN_ROUND_ROUTER_TIMEOUT_MS=${ROUTER_TIMEOUT_MS:-10000}
export OPENBITFUN_ROUND_ROUTER_RECENT_ROUNDS=${ROUTER_RECENT_ROUNDS:-3}
export OPENBITFUN_ROUND_ROUTER_MAX_INPUT_CHARS=${ROUTER_MAX_INPUT_CHARS:-80000}
export OPENBITFUN_ROUND_ROUTER_MAX_INPUT_TOKENS=${ROUTER_MAX_INPUT_TOKENS:-4096}
export OPENBITFUN_ROUND_ROUTER_CONTEXT_WINDOW=${ROUTER_CONTEXT_WINDOW:-33792}
export OPENBITFUN_ROUND_ROUTER_SUMMARY_ENABLED=${ROUTER_SUMMARY_ENABLED:-true}
export OPENBITFUN_ROUND_ROUTER_SUMMARY_MIN_ROUNDS=${ROUTER_SUMMARY_MIN_ROUNDS:-4}
export OPENBITFUN_ROUND_ROUTER_SUMMARY_TIMEOUT_MS=${ROUTER_SUMMARY_TIMEOUT_MS:-20000}
if [[ -n ${ROUTER_TOKENIZER_PATH:-} ]]; then
  export OPENBITFUN_ROUND_ROUTER_TOKENIZER_PATH=$ROUTER_TOKENIZER_PATH
fi
export OPENBITFUN_ROUND_ROUTER_SIMPLE_THRESHOLD=${ROUTER_SIMPLE_THRESHOLD:-0.7}

if [[ -n ${ROUTER_API_KEY:-} ]]; then
  export OPENBITFUN_ROUND_ROUTER_API_KEY=$ROUTER_API_KEY
else
  unset OPENBITFUN_ROUND_ROUTER_API_KEY
fi

if [[ -n ${ROUTER_TRACE_PATH:-} ]]; then
  export OPENBITFUN_ROUND_ROUTER_TRACE=$ROUTER_TRACE_PATH
fi

# 不添加参数时保持 OpenBitFun 原本的交互模式；传入 exec 等参数时则原样转发。
exec "$openbitfun_bin" "$@"
