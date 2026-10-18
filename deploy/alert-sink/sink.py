#!/usr/bin/env python3
"""Local alert sink (spec `grafana-dashboards`, "Provisioned alert rules").

Grafana's `discount-sink` contact point posts its webhook notifications here. Each one is
printed as a single JSON line (docker logs) and kept in memory; `GET /` returns the last
100, newest first, so the smoke script and a person can see what was delivered.
Standard library only; runs from python:3-alpine with this file mounted.
"""
import json
import sys
from collections import deque
from http.server import BaseHTTPRequestHandler, HTTPServer

LAST = deque(maxlen=100)


def summarize(n):
    return {
        "receivedAt": n.get("_receivedAt"),
        "status": n.get("status"),
        "alerts": [
            {
                "status": a.get("status"),
                "labels": a.get("labels", {}),
                "summary": a.get("annotations", {}).get("summary"),
                "startsAt": a.get("startsAt"),
                "endsAt": a.get("endsAt"),
            }
            for a in n.get("alerts", [])
        ],
    }


class Handler(BaseHTTPRequestHandler):
    def _json(self, code, body):
        data = json.dumps(body).encode()
        self.send_response(code)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(data)))
        self.end_headers()
        self.wfile.write(data)

    def do_GET(self):
        if self.path.startswith("/healthz"):
            return self._json(200, {"status": "ok", "notifications": len(LAST)})
        return self._json(200, list(LAST))

    def do_POST(self):
        length = int(self.headers.get("Content-Length") or 0)
        raw = self.rfile.read(length)
        try:
            n = json.loads(raw or b"{}")
        except json.JSONDecodeError:
            return self._json(400, {"error": "body is not JSON"})
        import datetime

        n["_receivedAt"] = datetime.datetime.now(datetime.timezone.utc).isoformat(timespec="seconds")
        s = summarize(n)
        LAST.appendleft(s)
        print(json.dumps(s), flush=True)
        return self._json(200, {"ok": True})

    def log_message(self, *_):  # the JSON line above is the log
        pass


if __name__ == "__main__":
    port = int(sys.argv[1]) if len(sys.argv) > 1 else 8080
    print(json.dumps({"listening": port}), flush=True)
    HTTPServer(("0.0.0.0", port), Handler).serve_forever()
