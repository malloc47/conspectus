#!/usr/bin/env python3
"""A local stand-in for a model gateway, for recording how a harness reports
a failed turn without spending quota or provoking a real failure.

    stub-gateway.py PORT SECONDS MODEFILE

Listens on 127.0.0.1:PORT for SECONDS seconds and answers every request with
an Anthropic-shaped error whose HTTP status is the number in MODEFILE (read on
each request, so a scenario can change it). It reads nothing from a request
and logs nothing: a harness sends its credential in the request headers, and
this ignores them. Run with `python3 -I`.
"""
import http.server
import json
import sys
import time

PORT, SECONDS, MODEFILE = int(sys.argv[1]), int(sys.argv[2]), sys.argv[3]
KINDS = {400: "invalid_request_error", 401: "authentication_error", 402: "billing_error",
         403: "permission_error", 429: "rate_limit_error", 500: "api_error", 529: "overloaded_error"}


class Handler(http.server.BaseHTTPRequestHandler):
    def _answer(self):
        length = int(self.headers.get("content-length") or 0)
        if length:
            self.rfile.read(min(length, 1 << 20))
        try:
            with open(MODEFILE) as handle:
                code = int(handle.read().strip())
        except (OSError, ValueError):
            code = 500
        body = json.dumps({"type": "error", "error": {"type": KINDS.get(code, "api_error"), "message": "stub"}}).encode()
        self.send_response(code)
        self.send_header("content-type", "application/json")
        self.send_header("content-length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    do_POST = do_GET = do_HEAD = _answer

    def log_message(self, *args):
        pass


server = http.server.ThreadingHTTPServer(("127.0.0.1", PORT), Handler)
server.timeout = 1
end = time.time() + SECONDS
while time.time() < end:
    server.handle_request()
