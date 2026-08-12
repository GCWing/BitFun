#!/usr/bin/env python3
"""Local OpenAI-compatible embedding server for gbrain (port 8890).

Model: Qdrant/bge-small-zh-v1.5 (ONNX, Dim=512) via onnxruntime + transformers
tokenizer. No optimum dependency (optimum-onnxruntime has no py3.14 wheel).

History (see .workbuddy/HANDBOOK.md):
  uvicorn/FastAPI -> wedges after ~58 requests (async + ONNX blocking)
  single-thread http.server -> queue timeouts under gbrain 20-way concurrency
  ThreadingHTTPServer -> stable (200/200 concurrent test passed)
"""

import json
import logging
import os
import sys
import time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from threading import BoundedSemaphore

import numpy as np
import onnxruntime as ort
from transformers import AutoTokenizer

MODEL_DIR = os.environ.get(
    "GBRAIN_EMBED_MODEL_DIR",
    os.path.expanduser(
        r"~/.cache/huggingface/hub/models--Qdrant--bge-small-zh-v1.5/snapshots/v1.5"
    ),
)
MODEL_FILE = os.environ.get("GBRAIN_EMBED_MODEL_FILE", "model_optimized.onnx")
HOST = os.environ.get("GBRAIN_EMBED_HOST", "127.0.0.1")
PORT = int(os.environ.get("GBRAIN_EMBED_PORT", "8890"))
MAX_TOKENS = 512
MAX_REQUEST_BYTES = 2 * 1024 * 1024  # 2 MiB hard cap for request bodies (d8-P2-5)
MAX_BATCH_SIZE = 128
# Cap concurrent /v1/embeddings requests. ThreadingHTTPServer spawns a thread
# per connection; without a bound a local process can open thousands of sockets
# and exhaust threads/memory (d8-P1-5). 8 >= gbrain's 20-way concurrency is
# sized below it; excess requests queue on the semaphore instead of stacking
# threads.
MAX_CONCURRENT_REQUESTS = 8
_concurrency_gate = BoundedSemaphore(MAX_CONCURRENT_REQUESTS)

logging.basicConfig(
    level=logging.INFO,
    format="[embed-server] %(message)s",
    stream=sys.stdout,
)


class EmbedServer:
    def __init__(self):
        logging.info("Loading BAAI/bge-small-zh-v1.5...")
        t0 = time.time()
        model_path = os.path.join(MODEL_DIR, MODEL_FILE)
        try:
            self.tokenizer = AutoTokenizer.from_pretrained(MODEL_DIR, local_files_only=True)
            self.sess = ort.InferenceSession(
                model_path,
                providers=["CPUExecutionProvider"],
            )
        except Exception as e:  # noqa: BLE001
            # Friendly, actionable diagnostics instead of a bare traceback
            # (d8-P2-3): the model path can be overridden via
            # GBRAIN_EMBED_MODEL_DIR / GBRAIN_EMBED_MODEL_FILE.
            logging.error(
                "Failed to load embedding model.\n"
                f"  model dir : {MODEL_DIR}\n"
                f"  model file: {model_path}\n"
                f"  error     : {e}\n"
                "Fix: set GBRAIN_EMBED_MODEL_DIR (and GBRAIN_EMBED_MODEL_FILE if the\n"
                "onnx file has a different name) to a local path containing the\n"
                "tokenizer files + the onnx model, e.g.\n"
                "  $env:GBRAIN_EMBED_MODEL_DIR='C:/models/bge-small-zh-v1.5'\n"
                "  $env:GBRAIN_EMBED_MODEL_FILE='model_optimized.onnx'"
            )
            raise
        self.input_names = [i.name for i in self.sess.get_inputs()]
        self.dim = 512
        logging.info(f"Model loaded. Dim={self.dim} ({(time.time() - t0):.1f}s)")

    def _mean_pool(self, last_hidden, mask):
        # mask must stay 2D for count; expanded copy only for weighting
        m = mask.astype("float32")[..., np.newaxis]  # (B, S, 1)
        summed = (last_hidden * m).sum(1)            # (B, D)
        count = mask.astype("float32").sum(1).clip(min=1e-9)[..., np.newaxis]  # (B, 1)
        return summed / count

    def embed(self, texts):
        enc = self.tokenizer(
            list(texts),
            padding=True,
            truncation=True,
            max_length=MAX_TOKENS,
            return_tensors="np",
        )
        feed = {}
        for name in self.input_names:
            if name in enc:
                feed[name] = enc[name]
        out = self.sess.run(None, feed)
        # last_hidden_state is the first output
        last_hidden = out[0]
        mask = enc["attention_mask"]
        pooled = self._mean_pool(last_hidden, mask).astype("float32")
        pooled = pooled / np.linalg.norm(pooled, axis=1, keepdims=True).clip(min=1e-9)
        # OpenAI format: each item's embedding is a flat list (no batch axis).
        return pooled.tolist()


class Handler(BaseHTTPRequestHandler):
    server: "EmbedServerWrapper"  # type: ignore
    timeout = 60

    def log_message(self, fmt, *args):
        try:
            msg = fmt % args
        except Exception:  # noqa: BLE001
            msg = fmt
        logging.info(f"{self.command} {self.path} HTTP/1.1 {msg}")

    def do_GET(self):
        if self.path == "/health":
            body = b'{"status":"ok"}'
            self.send_response(200)
            self.send_header("Content-Type", "application/json")
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)
            return
        self.send_error(404)

    def do_POST(self):
        if self.path != "/v1/embeddings":
            self.send_error(404)
            return
        try:
            length = int(self.headers.get("Content-Length", 0))
        except (TypeError, ValueError):
            # Non-numeric Content-Length: 4xx instead of a broken connection
            # (d8-P2-4).
            self._json(400, {"error": {"message": "invalid Content-Length", "type": "invalid_request_error"}})
            return
        if length < 0 or length > MAX_REQUEST_BYTES:
            self._json(413, {"error": {"message": f"request body too large (limit {MAX_REQUEST_BYTES} bytes)", "type": "invalid_request_error"}})
            return
        raw = self.rfile.read(length)
        try:
            req = json.loads(raw)
        except json.JSONDecodeError:
            self._json(400, {"error": {"message": "invalid JSON", "type": "invalid_request_error"}})
            return
        inp = req.get("input", "")
        if isinstance(inp, str):
            texts = [inp]
        elif isinstance(inp, list):
            if not inp:
                # Explicit definition for an empty batch (d8-P2-2): refuse
                # rather than feeding the tokenizer an empty batch.
                self._json(400, {"error": {"message": "input list must not be empty", "type": "invalid_request_error"}})
                return
            if len(inp) > MAX_BATCH_SIZE:
                self._json(400, {"error": {"message": f"input list too large (max {MAX_BATCH_SIZE} items)", "type": "invalid_request_error"}})
                return
            texts = [t if isinstance(t, str) else str(t) for t in inp]
        else:
            self._json(400, {"error": {"message": "input must be string or list", "type": "invalid_request_error"}})
            return
        try:
            # Bound concurrent embedding work; excess requests wait on the
            # semaphore instead of piling up threads (d8-P1-5).
            with _concurrency_gate:
                vectors = self.server.embedder.embed(texts)
        except Exception as e:  # noqa: BLE001
            logging.error(f"embed failed: {e}")
            self._json(500, {"error": {"message": str(e), "type": "server_error"}})
            return
        data = [{"object": "embedding", "index": i, "embedding": v} for i, v in enumerate(vectors)]
        self._json(200, {"object": "list", "data": data, "model": req.get("model", "bge-small-zh-v1.5"),
                         "usage": {"prompt_tokens": 0, "total_tokens": 0}})

    def _json(self, code, payload):
        body = json.dumps(payload).encode("utf-8")
        self.send_response(code)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)


class EmbedServerWrapper(ThreadingHTTPServer):
    daemon_threads = True

    def __init__(self, server_address, handler_class):
        self.embedder = EmbedServer()
        super().__init__(server_address, handler_class)


if __name__ == "__main__":
    srv = EmbedServerWrapper((HOST, PORT), Handler)
    logging.info(f"Embedding server running on http://{HOST}:{PORT}")
    try:
        srv.serve_forever()
    except KeyboardInterrupt:
        pass
