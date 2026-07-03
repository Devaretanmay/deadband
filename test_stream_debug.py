import json
from http.server import HTTPServer, BaseHTTPRequestHandler
import time

TOOL_CALL_CHUNKS = [
    'data: {"choices":[{"index":0,"delta":{"role":"assistant","tool_calls":[{"index":0,"function":{"name":"search","arguments":""}}]}}]}\n\n',
    'data: {"choices":[{"index":0,"delta":{"tool_calls":[{"index":0,"function":{"arguments":"{\\"q\\":\\"test\\"}"}}]}}]}\n\n',
    'data: {"choices":[{"index":0,"delta":{}}]}\n\n',
    'data: {"choices":[{"index":0,"delta":{"role":"assistant","tool_calls":[{"index":0,"function":{"name":"search","arguments":""}}]}}]}\n\n',
    'data: {"choices":[{"index":0,"delta":{"tool_calls":[{"index":0,"function":{"arguments":"{\\"q\\":\\"test2\\"}"}}]}}]}\n\n',
    'data: {"choices":[{"index":0,"delta":{}}]}\n\n',
    'data: {"choices":[{"index":0,"delta":{"role":"assistant","tool_calls":[{"index":0,"function":{"name":"search","arguments":""}}]}}]}\n\n',
    'data: {"choices":[{"index":0,"delta":{"tool_calls":[{"index":0,"function":{"arguments":"{\\"q\\":\\"test3\\"}"}}]}}]}\n\n',
    'data: [DONE]\n\n',
]

class Handler(BaseHTTPRequestHandler):
    def do_POST(self):
        body_len = int(self.headers.get("Content-Length", 0))
        body = self.rfile.read(body_len)
        print(f"MOCK: {self.path} body={body[:100]}", flush=True)

        self.send_response(200)
        self.send_header("Content-Type", "text/event-stream")
        self.send_header("Cache-Control", "no-cache")
        self.send_header("Connection", "close")
        self.end_headers()

        for chunk in TOOL_CALL_CHUNKS:
            self.wfile.write(chunk.encode())
            self.wfile.flush()
            time.sleep(0.05)

        # Don't close explicitly - let connection close naturally
        print(f"MOCK: done sending {len(TOOL_CALL_CHUNKS)} chunks", flush=True)

    def log_message(self, format, *args):
        pass

print("Starting debug mock server on port 8001...", flush=True)
HTTPServer(("", 8001), Handler).serve_forever()
