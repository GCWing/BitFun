#!/bin/sh
set -eu

SCRIPT_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
IOS_DIR=$(CDPATH= cd -- "$SCRIPT_DIR/.." && pwd)
OUTPUT_DIR=$(mktemp -d "${TMPDIR:-/tmp}/bitfun-ios-pure-swift-tests.XXXXXX")
trap 'rm -rf "$OUTPUT_DIR"' EXIT HUP INT TERM

xcrun --sdk macosx swiftc \
  "$IOS_DIR/BitFun/Infrastructure/RemoteAuthorityGate.swift" \
  "$SCRIPT_DIR/RemoteAuthorityGateTests.swift" \
  -o "$OUTPUT_DIR/remote-authority-gate-tests"
"$OUTPUT_DIR/remote-authority-gate-tests"

xcrun --sdk macosx swiftc \
  "$IOS_DIR/BitFun/Infrastructure/AccountFailureCopy.swift" \
  "$SCRIPT_DIR/AccountFailureCopyTests.swift" \
  -o "$OUTPUT_DIR/account-failure-copy-tests"
"$OUTPUT_DIR/account-failure-copy-tests"

printf '%s\n' 'iOS pure Swift focused tests passed.'
