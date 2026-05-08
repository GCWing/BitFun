#!/bin/bash
# Test script for BitFun ACP server
# This script demonstrates basic ACP protocol interaction

echo "=== BitFun ACP Server Test ==="
echo ""

BINARY="${BITFUN_CLI:-target/debug/bitfun-cli}"
WORKSPACE="/tmp/test-acp"
PIPE_DIR="$(mktemp -d /tmp/bitfun-acp-test-sh.XXXXXX)"
ACP_IN="$PIPE_DIR/in"
ACP_OUT="$PIPE_DIR/out"
mkdir -p "$WORKSPACE"
mkfifo "$ACP_IN" "$ACP_OUT"

cleanup() {
  exec 3>&- 2>/dev/null || true
  exec 4<&- 2>/dev/null || true
  if [[ -n "${ACP_PID:-}" ]]; then
    kill "$ACP_PID" 2>/dev/null || true
    wait "$ACP_PID" 2>/dev/null || true
  fi
  rm -rf "$PIPE_DIR"
}
trap cleanup EXIT

echo "Test 1: Initialize"
echo "Test 2: Create Session"
echo "Test 3: List Sessions"
"$BINARY" acp <"$ACP_IN" >"$ACP_OUT" &
ACP_PID="$!"
exec 3>"$ACP_IN"
exec 4<"$ACP_OUT"

printf '%s\n' \
  '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":1,"clientCapabilities":{"fs":{"readTextFile":true,"writeTextFile":true},"terminal":true},"clientInfo":{"name":"TestClient","version":"1.0"}}}' \
  >&3

responses=0
while [[ "$responses" -lt 3 ]]; do
  if ! IFS= read -r -t 15 line <&4; then
    echo "Timed out waiting for ACP response" >&2
    exit 1
  fi

  echo "$line"
  if [[ "$line" == *'"id":'* ]]; then
    responses=$((responses + 1))
  fi

  if [[ "$line" == *'"id":1'* ]]; then
    printf '%s\n' \
      "{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"session/new\",\"params\":{\"cwd\":\"$WORKSPACE\",\"mcpServers\":[]}}" \
      >&3
  elif [[ "$line" == *'"id":2'* ]]; then
    printf '%s\n' \
      "{\"jsonrpc\":\"2.0\",\"id\":3,\"method\":\"session/list\",\"params\":{\"cwd\":\"$WORKSPACE\"}}" \
      >&3
  fi
done
exec 3>&-
echo ""

echo "=== Tests Complete ==="
echo ""
echo "Note: This is a basic test of the typed ACP protocol layer."
