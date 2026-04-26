#!/bin/bash
# Test script for BitFun ACP server
# This script demonstrates basic ACP protocol interaction

echo "=== BitFun ACP Server Test ==="
echo ""

# Check if bitfun-cli is built
if ! command -v bitfun-cli &> /dev/null; then
    echo "Error: bitfun-cli not found in PATH"
    echo "Please build the CLI first: cargo build --package bitfun-cli"
    exit 1
fi

echo "Test 1: Initialize"
echo "Sending: initialize request"
echo '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":1,"clientCapabilities":{"fs":{"readTextFile":true,"writeTextFile":true},"terminal":true},"clientInfo":{"name":"TestClient","version":"1.0"}}}' | bitfun-cli acp
echo ""

echo "Test 2: Create Session"
echo "Sending: session/new request"
echo '{"jsonrpc":"2.0","id":2,"method":"session/new","params":{"cwd":"/tmp/test-acp"}}' | bitfun-cli acp
echo ""

echo "Test 3: List Tools"
echo "Sending: tools/list request"
echo '{"jsonrpc":"2.0","id":3,"method":"tools/list"}' | bitfun-cli acp
echo ""

echo "Test 4: List Sessions"
echo "Sending: session/list request"
echo '{"jsonrpc":"2.0","id":4,"method":"session/list"}' | bitfun-cli acp
echo ""

echo "=== Tests Complete ==="
echo ""
echo "Note: This is a basic test of the protocol layer."
echo "Full agentic workflow execution is not yet implemented."