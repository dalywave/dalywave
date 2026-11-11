#!/usr/bin/env python3
"""Маленький HTTP-сервер с поддержкой Range для тестового аудио.

http.server из стандартной библиотеки Range не умеет, а движку он нужен: чтение с середины
файла, «хвост» файла (bytes=-N) и продолжение загрузки после обрыва.

Поддерживает: GET/HEAD, Range bytes=a-b | a- | -n, 206/416, Accept-Ranges, CORS.
Для замеров на «мобильной» сети: --latency-ms (задержка перед ответом, имитация RTT)
и --kbps (ограничение скорости отдачи тела).

Запуск: python3 tools/serve.py --dir testdata --port 8420 [--latency-ms 80 --kbps 4000]
"""

import argparse
import mimetypes
import os
import re
import sys
import time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

mimetypes.add_type("audio/ogg", ".opus")
mimetypes.add_type("audio/ogg", ".ogg")
mimetypes.add_type("audio/mp4", ".m4a")
mimetypes.add_type("audio/mp4", ".mp4")
mimetypes.add_type("audio/flac", ".flac")
mimetypes.add_type("audio/mpeg", ".mp3")
mimetypes.add_type("audio/webm", ".webm")

RANGE_RE = re.compile(r"^bytes=(\d*)-(\d*)$")


class Handler(BaseHTTPRequestHandler):
    protocol_version = "HTTP/1.1"
    root = "."
    latency = 0.0
    kbps = 0

    def handle(self):
        # Клиент закрывает соединение, когда байты уже есть (хвост скачан параллельно) — это норма.
        try:
            super().handle()
        except (ConnectionResetError, BrokenPipeError):
            pass

    def log_request(self, code="-", size="-"):
        pass

    def log_message(self, fmt, *args):
        sys.stdout.write("%s %s\n" % (time.strftime("%H:%M:%S"), fmt % args))
        sys.stdout.flush()

    def _resolve(self):
        path = self.path.split("?", 1)[0]
        full = os.path.realpath(os.path.join(self.root, path.lstrip("/")))
        if not full.startswith(os.path.realpath(self.root)) or not os.path.isfile(full):
            return None
        return full

    def _common_headers(self):
        self.send_header("Accept-Ranges", "bytes")
        self.send_header("Access-Control-Allow-Origin", "*")
        self.send_header("Access-Control-Expose-Headers", "Content-Range, Content-Length, Accept-Ranges")
        self.send_header("Cache-Control", "no-store")

    def do_OPTIONS(self):
        self.send_response(204)
        self._common_headers()
        self.send_header("Access-Control-Allow-Headers", "Range")
        self.send_header("Content-Length", "0")
        self.end_headers()

    def do_HEAD(self):
        self._serve(head=True)

    def do_GET(self):
        self._serve(head=False)

    def _serve(self, head):
        if self.latency:
            time.sleep(self.latency)
        full = self._resolve()
        if full is None:
            self.send_response(404)
            self._common_headers()
            self.send_header("Content-Length", "0")
            self.end_headers()
            return
        size = os.path.getsize(full)
        start, end, status = 0, size - 1, 200
        rng = self.headers.get("Range")
        if rng:
            m = RANGE_RE.match(rng.strip())
            if not m or (m.group(1) == "" and m.group(2) == ""):
                return self._unsatisfiable(size)
            a, b = m.group(1), m.group(2)
            if a == "":
                n = int(b)
                if n == 0:
                    return self._unsatisfiable(size)
                start, end = max(0, size - n), size - 1
            else:
                start = int(a)
                end = min(int(b), size - 1) if b else size - 1
                if start >= size or start > end:
                    return self._unsatisfiable(size)
            status = 206
        length = end - start + 1
        self.send_response(status)
        self._common_headers()
        self.send_header("Content-Type", mimetypes.guess_type(full)[0] or "application/octet-stream")
        self.send_header("Content-Length", str(length))
        if status == 206:
            self.send_header("Content-Range", f"bytes {start}-{end}/{size}")
        self.end_headers()
        if head:
            return
        chunk = 16 * 1024
        sent = 0
        t0 = time.monotonic()
        try:
            with open(full, "rb") as f:
                f.seek(start)
                while sent < length:
                    data = f.read(min(chunk, length - sent))
                    if not data:
                        break
                    self.wfile.write(data)
                    sent += len(data)
                    if self.kbps:
                        # Ограничение скорости: ждём, пока «положенное» время не догонит отправленное.
                        due = sent * 8 / (self.kbps * 1000)
                        lag = due - (time.monotonic() - t0)
                        if lag > 0:
                            time.sleep(lag)
        except (BrokenPipeError, ConnectionResetError):
            pass
        finally:
            self.log_message('"%s" %s range=%s sent=%d/%d %.0fms', self.requestline, status,
                             rng or "-", sent, length, (time.monotonic() - t0) * 1000)

    def _unsatisfiable(self, size):
        self.send_response(416)
        self._common_headers()
        self.send_header("Content-Range", f"bytes */{size}")
        self.send_header("Content-Length", "0")
        self.end_headers()


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--dir", default=os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "testdata"))
    ap.add_argument("--port", type=int, default=8420)
    ap.add_argument("--bind", default="0.0.0.0")
    ap.add_argument("--latency-ms", type=float, default=0)
    ap.add_argument("--kbps", type=int, default=0)
    args = ap.parse_args()
    Handler.root = os.path.abspath(args.dir)
    Handler.latency = args.latency_ms / 1000
    Handler.kbps = args.kbps
    srv = ThreadingHTTPServer((args.bind, args.port), Handler)
    srv.daemon_threads = True
    print(f"serving {Handler.root} on http://{args.bind}:{args.port} latency={args.latency_ms}ms kbps={args.kbps or 'inf'}")
    sys.stdout.flush()
    srv.serve_forever()


if __name__ == "__main__":
    main()
