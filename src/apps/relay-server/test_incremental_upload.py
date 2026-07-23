#!/usr/bin/env python3
"""
Test script for the incremental web file upload feature on the relay server.

Tests:
  1. Create room via WebSocket (keep connection alive)
  2. Legacy full upload (upload-web) with global content store
  3. check-web-files: all files should already exist
  4. check-web-files: 2 existing + 1 new
  5. upload-web-files: only the needed file
  6. Serve uploaded files via /r/{room_id}/
  7. Second room reuses global store — zero uploads needed
  8. Hash mismatch rejection

Usage:
    python3 test_incremental_upload.py [relay_url]
"""

import asyncio
import base64
import hashlib
import json
import sys
import time
import urllib.request
import urllib.error

try:
    import websockets
except ImportError:
    print("Installing websockets...")
    import subprocess
    subprocess.check_call([sys.executable, "-m", "pip", "install", "websockets", "-q"])
    import websockets

RELAY_URL = sys.argv[1] if len(sys.argv) > 1 else "https://remote.openbitfun.com/relay"
WS_URL = RELAY_URL.replace("http://", "ws://").replace("https://", "wss://") + "/ws"

PASS = 0
FAIL = 0


def green(s):
    print(f"\033[32m  PASS: {s}\033[0m")


def red(s):
    print(f"\033[31m  FAIL: {s}\033[0m")


def assert_eq(desc, expected, actual):
    global PASS, FAIL
    if expected == actual:
        green(desc)
        PASS += 1
    else:
        red(f"{desc} (expected={expected!r}, actual={actual!r})")
        FAIL += 1


def assert_contains(desc, haystack, needle):
    global PASS, FAIL
    if needle in str(haystack):
        green(desc)
        PASS += 1
    else:
        red(f"{desc} (expected to contain {needle!r}, got: {haystack!r})")
        FAIL += 1


def b64enc(s: str) -> str:
    return base64.b64encode(s.encode()).decode()


def sha256hex(s: str) -> str:
    return hashlib.sha256(s.encode()).hexdigest()


def http_get(url: str) -> tuple:
    """Returns (status_code, body_text)."""
    try:
        req = urllib.request.Request(url)
        with urllib.request.urlopen(req, timeout=10) as resp:
            return resp.status, resp.read().decode()
    except urllib.error.HTTPError as e:
        return e.code, e.read().decode() if e.fp else ""
    except Exception as e:
        return 0, str(e)


def http_post_json(url: str, data: dict) -> tuple:
    """Returns (status_code, parsed_json_or_None)."""
    try:
        body = json.dumps(data).encode()
        req = urllib.request.Request(url, data=body, method="POST")
        req.add_header("Content-Type", "application/json")
        with urllib.request.urlopen(req, timeout=10) as resp:
            text = resp.read().decode()
            try:
                return resp.status, json.loads(text)
            except json.JSONDecodeError:
                return resp.status, text
    except urllib.error.HTTPError as e:
        text = e.read().decode() if e.fp else ""
        try:
            return e.code, json.loads(text)
        except (json.JSONDecodeError, Exception):
            return e.code, text
    except Exception as e:
        return 0, str(e)


async def create_room_ws(room_id: str):
    """Create a room via WebSocket and return the connection (kept alive)."""
    ws = await websockets.connect(WS_URL)
    await ws.send(json.dumps({
        "type": "create_room",
        "room_id": room_id,
        "device_id": f"test-{room_id}",
        "device_type": "desktop",
        "public_key": "dGVzdA==",
    }))
    resp = await asyncio.wait_for(ws.recv(), timeout=5)
    data = json.loads(resp)
    return ws, data


async def run_tests():
    global PASS, FAIL

    ts = int(time.time())
    room1 = f"test_incr_{ts}_1"
    room2 = f"test_incr_{ts}_2"

    # Test data
    file1_content = "<html><body>Hello BitFun Test</body></html>"
    file2_content = "body { margin: 0; background: #1a1a2e; }"
    file3_content = "console.log('BitFun incremental upload test');"

    file1_b64 = b64enc(file1_content)
    file2_b64 = b64enc(file2_content)
    file3_b64 = b64enc(file3_content)

    file1_hash = sha256hex(file1_content)
    file2_hash = sha256hex(file2_content)
    file3_hash = sha256hex(file3_content)

    file1_size = len(file1_content)
    file2_size = len(file2_content)
    file3_size = len(file3_content)

    print("=" * 50)
    print("  Incremental Upload Test Suite")
    print(f"  Relay: {RELAY_URL}")
    print(f"  WS:    {WS_URL}")
    print("=" * 50)
    print()

    # ── [0] Health check ──
    print("[0] Health check")
    status, body = http_get(f"{RELAY_URL}/health")
    assert_eq("Server is healthy", 200, status)
    assert_contains("Response contains 'healthy'", body, "healthy")
    print()

    print("Test files prepared:")
    print(f"  index.html       hash={file1_hash[:12]}... size={file1_size}")
    print(f"  assets/style.css hash={file2_hash[:12]}... size={file2_size}")
    print(f"  assets/app.js    hash={file3_hash[:12]}... size={file3_size}")
    print()

    # ── [1] Create rooms ──
    print("[1] Create rooms via WebSocket (kept alive)")
    ws1, data1 = await create_room_ws(room1)
    assert_eq(f"Room 1 ({room1}) created", "room_created", data1.get("type"))

    ws2, data2 = await create_room_ws(room2)
    assert_eq(f"Room 2 ({room2}) created", "room_created", data2.get("type"))
    print()

    try:
        # ── [2] Legacy full upload ──
        print("[2] Legacy full upload (upload-web) to Room 1")
        status, resp = http_post_json(
            f"{RELAY_URL}/api/rooms/{room1}/upload-web",
            {"files": {"index.html": file1_b64, "assets/style.css": file2_b64}},
        )
        assert_eq("HTTP 200", 200, status)
        assert_eq("Status is ok", "ok", resp.get("status") if isinstance(resp, dict) else "")
        assert_eq("2 new files written", 2, resp.get("files_written", 0) if isinstance(resp, dict) else 0)
        assert_eq("0 files reused (first upload)", 0, resp.get("files_reused", 0) if isinstance(resp, dict) else -1)
        print()

        # ── [3] Serve uploaded files ──
        print("[3] Serve uploaded files via /r/{room_id}/")
        status, html = http_get(f"{RELAY_URL}/r/{room1}/index.html")
        assert_eq("index.html status 200", 200, status)
        assert_eq("index.html content correct", file1_content, html)

        status, css = http_get(f"{RELAY_URL}/r/{room1}/assets/style.css")
        assert_eq("style.css status 200", 200, status)
        assert_eq("style.css content correct", file2_content, css)
        print()

        # ── [4] check-web-files: all exist ──
        print("[4] check-web-files: all files should already exist in store")
        status, resp = http_post_json(
            f"{RELAY_URL}/api/rooms/{room1}/check-web-files",
            {
                "files": [
                    {"path": "index.html", "hash": file1_hash, "size": file1_size},
                    {"path": "assets/style.css", "hash": file2_hash, "size": file2_size},
                ]
            },
        )
        assert_eq("HTTP 200", 200, status)
        needed = resp.get("needed", []) if isinstance(resp, dict) else []
        assert_eq("0 files needed", 0, len(needed))
        assert_eq("2 files exist", 2, resp.get("existing_count", 0) if isinstance(resp, dict) else 0)
        assert_eq("Total count 2", 2, resp.get("total_count", 0) if isinstance(resp, dict) else 0)
        print()

        # ── [5] check-web-files: 2 existing + 1 new ──
        print("[5] check-web-files: 2 existing + 1 new file")
        status, resp = http_post_json(
            f"{RELAY_URL}/api/rooms/{room1}/check-web-files",
            {
                "files": [
                    {"path": "index.html", "hash": file1_hash, "size": file1_size},
                    {"path": "assets/style.css", "hash": file2_hash, "size": file2_size},
                    {"path": "assets/app.js", "hash": file3_hash, "size": file3_size},
                ]
            },
        )
        assert_eq("HTTP 200", 200, status)
        needed = resp.get("needed", []) if isinstance(resp, dict) else []
        assert_eq("1 file needed", 1, len(needed))
        assert_eq("2 files exist", 2, resp.get("existing_count", 0) if isinstance(resp, dict) else 0)
        assert_eq("Needed file is assets/app.js", "assets/app.js", needed[0] if needed else "")
        print()

        # ── [6] upload-web-files: only needed ──
        print("[6] Upload only the needed file via upload-web-files")
        status, resp = http_post_json(
            f"{RELAY_URL}/api/rooms/{room1}/upload-web-files",
            {
                "files": {
                    "assets/app.js": {
                        "content": file3_b64,
                        "hash": file3_hash,
                    }
                }
            },
        )
        assert_eq("HTTP 200", 200, status)
        assert_eq("Status is ok", "ok", resp.get("status") if isinstance(resp, dict) else "")
        assert_eq("1 file stored", 1, resp.get("files_stored", 0) if isinstance(resp, dict) else 0)
        print()

        # ── [7] Verify newly uploaded file ──
        print("[7] Verify newly uploaded file is served")
        status, js = http_get(f"{RELAY_URL}/r/{room1}/assets/app.js")
        assert_eq("app.js status 200", 200, status)
        assert_eq("app.js content correct", file3_content, js)
        print()

        # ── [8] Second room: all files reused ──
        print("[8] Room 2: check-web-files should find all 3 in global store")
        status, resp = http_post_json(
            f"{RELAY_URL}/api/rooms/{room2}/check-web-files",
            {
                "files": [
                    {"path": "index.html", "hash": file1_hash, "size": file1_size},
                    {"path": "assets/style.css", "hash": file2_hash, "size": file2_size},
                    {"path": "assets/app.js", "hash": file3_hash, "size": file3_size},
                ]
            },
        )
        assert_eq("HTTP 200", 200, status)
        needed = resp.get("needed", []) if isinstance(resp, dict) else []
        assert_eq("0 files needed (all reused)", 0, len(needed))
        assert_eq("3 files exist in global store", 3, resp.get("existing_count", 0) if isinstance(resp, dict) else 0)
        print()

        # ── [9] Room 2 files served via symlinks ──
        print("[9] Room 2: files served correctly via symlinks")
        status, html2 = http_get(f"{RELAY_URL}/r/{room2}/index.html")
        assert_eq("Room 2 index.html correct", file1_content, html2)

        status, css2 = http_get(f"{RELAY_URL}/r/{room2}/assets/style.css")
        assert_eq("Room 2 style.css correct", file2_content, css2)

        status, js2 = http_get(f"{RELAY_URL}/r/{room2}/assets/app.js")
        assert_eq("Room 2 app.js correct", file3_content, js2)
        print()

        # ── [10] Hash mismatch rejection ──
        print("[10] Upload with wrong hash should be rejected")
        status, resp = http_post_json(
            f"{RELAY_URL}/api/rooms/{room1}/upload-web-files",
            {
                "files": {
                    "bad.js": {
                        "content": file3_b64,
                        "hash": "0" * 64,
                    }
                }
            },
        )
        assert_eq("Wrong hash returns 400", 400, status)
        print()

        # ── [11] check-web-files on nonexistent room ──
        print("[11] check-web-files on nonexistent room should return 404")
        status, resp = http_post_json(
            f"{RELAY_URL}/api/rooms/nonexistent_room/check-web-files",
            {"files": [{"path": "a.html", "hash": "abc", "size": 1}]},
        )
        assert_eq("Nonexistent room returns 404", 404, status)
        print()

    finally:
        await ws1.close()
        await ws2.close()

    # ── Summary ──
    print("=" * 50)
    total = PASS + FAIL
    if FAIL == 0:
        print(f"\033[32m  All {total} tests passed!\033[0m")
    else:
        print(f"  Results: \033[32m{PASS} passed\033[0m, \033[31m{FAIL} failed\033[0m")
    print("=" * 50)

    return FAIL == 0


if __name__ == "__main__":
    ok = asyncio.run(run_tests())
    sys.exit(0 if ok else 1)
