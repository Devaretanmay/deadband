import json
from http.server import HTTPServer, BaseHTTPRequestHandler

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
    def do_OPTIONS(self):
        self.send_response(200)
        self.send_header("Content-Type", "text/event-stream")
        self.send_header("Access-Control-Allow-Origin", "*")
        self.end_headers()

    def do_GET(self):
        self.send_response(200)
        self.send_header("Content-Type", "text/event-stream")
        self.send_header("Cache-Control", "no-cache")
        self.send_header("Access-Control-Allow-Origin", "*")
        self.end_headers()
        for chunk in TOOL_CALL_CHUNKS:
            self.wfile.write(chunk.encode())
            self.wfile.flush()

    def do_POST(self):
        body = self.rfile.read(int(self.headers.get("Content-Length", 0)))
        print(f"MOCK UPSTREAM REQUEST: {self.path}")
        print(f"  Body: {body[:200]}...")

        self.send_response(200)
        self.send_header("Content-Type", "text/event-stream")
        self.send_header("Cache-Control", "no-cache")
        self.send_header("Access-Control-Allow-Origin", "*")
        self.end_headers()

        for chunk in TOOL_CALL_CHUNKS:
            self.wfile.write(chunk.encode())
            self.wfile.flush()

    def log_message(self, format, *args):
        pass

print("Starting mock upstream server on port 8000...")
HTTPServer(("", 8000), Handler).serve_forever()
