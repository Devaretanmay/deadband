#!/bin/bash
set -e

# Cleanup function
cleanup() {
    kill $PROXY_PID $MOCK_PID 2>/dev/null
    wait $PROXY_PID $MOCK_PID 2>/dev/null
}
trap cleanup EXIT

PROJECT_DIR="/Users/tanmaydevare/Tanmay/deadband"

# Kill any existing processes on our ports
kill $(lsof -t -i:4399 -i:8000 2>/dev/null) 2>/dev/null || true
sleep 1

# Set upstream URL for proxy
echo 'http://localhost:8000/v1' > /Users/tanmaydevare/.deadband/upstream_url.txt

# Clear old stats
rm -f /Users/tanmaydevare/.deadband/stats.json

# Start mock server
cd "$PROJECT_DIR"
python3 test_mock_upstream.py &
MOCK_PID=$!
sleep 1
echo "MOCK_PID=$MOCK_PID"

# Verify mock server is up
curl -s --max-time 3 http://localhost:8000/v1/chat/completions \
    -H 'Content-Type: application/json' \
    -d '{"model":"x","messages":[{"role":"user","content":"hi"}]}' > /dev/null 2>&1
echo "Mock server: OK"

# Start proxy
cd "$PROJECT_DIR"
RUST_LOG=info ./target/release/deadband proxy --port 4399 --config deadband.yaml &
PROXY_PID=$!
sleep 2
echo "PROXY_PID=$PROXY_PID"

# Verify proxy is up
curl -s --max-time 3 -o /dev/null -w '%{http_code}' http://localhost:4399/ > /dev/null 2>&1
echo "Proxy: OK"

# Run the streaming test
echo ""
echo "=== STREAMING TEST WITH TOOL CALLS ==="
curl -s --max-time 15 -N http://localhost:4399/v1/chat/completions \
    -H 'Content-Type: application/json' \
    -H 'Authorization: Bearer test-key' \
    --data-raw '{"model":"gpt-4","messages":[{"role":"user","content":"search for something"}],"stream":true,"tools":[{"type":"function","function":{"name":"search","parameters":{"type":"object","properties":{"q":{"type":"string"}}}}}],"tool_choice":"auto"}' 2>&1

echo ""
echo ""
echo "=== STATS ==="
cat /Users/tanmaydevare/.deadband/stats.json 2>/dev/null || echo "NO_STATS"

echo ""
echo "=== TEST COMPLETE ==="
