"""Serves the picker on localhost and records what's decided in it, one JSON object per line,
appended to the decisions file. The latest line for a texture is what counts.

Usage: python3 -I server.py <page dir> <decisions.jsonl> [port]
"""
import http.server
import json
import os
import sys
import time

root, decisions = os.path.abspath(sys.argv[1]), os.path.abspath(sys.argv[2])
port = int(sys.argv[3]) if len(sys.argv) > 3 else 8765


class Handler(http.server.SimpleHTTPRequestHandler):
    def __init__(self, *a, **kw):
        super().__init__(*a, directory=root, **kw)

    def log_message(self, *a):
        pass

    def do_GET(self):
        if self.path.split("?")[0] != "/decisions.jsonl":
            return super().do_GET()
        data = open(decisions, "rb").read() if os.path.exists(decisions) else b""
        self.send_response(200)
        self.send_header("Content-Type", "application/x-ndjson")
        self.send_header("Cache-Control", "no-store")
        self.end_headers()
        self.wfile.write(data)

    def do_POST(self):
        if self.path != "/submit":
            self.send_error(404)
            return
        body = self.rfile.read(min(int(self.headers.get("Content-Length", 0)), 1 << 16))
        try:
            d = json.loads(body)
            entry = {
                "time": time.strftime("%Y-%m-%dT%H:%M:%S"),
                "target": str(d["target"]),
                "kind": str(d["kind"]),
                "pick": str(d.get("pick", "")),
                "note": str(d.get("note", ""))[:500],
            }
        except Exception:
            self.send_error(400)
            return
        with open(decisions, "a") as f:
            f.write(json.dumps(entry) + "\n")
        print("DECIDED " + json.dumps(entry), flush=True)
        self.send_response(204)
        self.end_headers()


print(f"serving http://127.0.0.1:{port}/", flush=True)
http.server.ThreadingHTTPServer(("127.0.0.1", port), Handler).serve_forever()
