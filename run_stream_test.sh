#!/bin/bash
set +e

PROJECT_DIR="/Users/tanmaydevare/Tanmay/deadband"
RESULT_FILE="/tmp/deadband_test_result.txt"

echo "test starting" > "$RESULT_FILE"

cleanup() {
    echo "cleaning up..." >> "$RESULT_FILE"
    kill $MOCK_PID $PROXY_PID 2>/dev/null
    wait $MOCK_PID $PROXY_PID 2>/dev/null
}

kill -9 $(lsof -t -i:8000 -i:4399 2>/dev/null) 2>/dev/null || true
sleep 1

echo 'http://localhost:8000/v1' > /Users/tanmaydevare/.deadband/upstream_url.txt
rm -f /Users/tanmaydevare/.deadband/stats.json

cd "$PROJECT_DIR"

nohup python3 test_mock_upstream.py > /tmp/mock_server.log 2>&1 &
MOCK_PID=$!
echo "mock PID=$MOCK_PID" >> "$RESULT_FILE"
sleep 1

nohup ./target/release/deadband proxy --port 4399 --config deadband.yaml > /tmp/proxy_debug.log 2>&1 &
PROXY_PID=$!
echo "proxy PID=$PROXY_PID" >> "$RESULT_FILE"
sleep 3

echo "=== MOCK CHECK ===" >> "$RESULT_FILE"
curl -s --max-time 3 http://localhost:8000/v1/chat/completions \
    -H 'Content-Type: application/json' \
    -d '{"model":"x","messages":[{"role":"user","content":"hi"}]}' 2>&1 | head -3 >> "$RESULT_FILE"

echo "=== MOCK CHECK DONE ===" >> "$RESULT_FILE"

echo "=== PROXY CHECK ===" >> "$RESULT_FILE"
curl -s --max-time 3 -o /dev/null -w "proxy HTTP status: %{http_code}" http://localhost:4399/ 2>&1 >> "$RESULT_FILE"
echo "" >> "$RESULT_FILE"

echo "=== STREAMING TEST ===" >> "$RESULT_FILE"
curl -s --max-time 10 http://localhost:4399/v1/chat/completions \
    -H 'Content-Type: application/json' \
    -H 'Authorization: Bearer test-key' \
    --data-raw '{"model":"gpt-4","messages":[{"role":"user","content":"search for something"}],"stream":true,"tools":[{"type":"function","function":{"name":"search","parameters":{"type":"object","properties":{"q":{"type":"string"}}}}}],"tool_choice":"auto"}' > /tmp/stream_result.txt 2>/tmp/stream_result_err.txt

echo "Streaming curl exit code: $?" >> "$RESULT_FILE"
echo "Streaming output:" >> "$RESULT_FILE"
cat /tmp/stream_result.txt >> "$RESULT_FILE"
echo "" >> "$RESULT_FILE"
echo "Streaming stderr:" >> "$RESULT_FILE"
cat /tmp/stream_result_err.txt >> "$RESULT_FILE"
echo "" >> "$RESULT_FILE"

echo "=== STATS ===" >> "$RESULT_FILE"
cat /Users/tanmaydevare/.deadband/stats.json 2>/dev/null >> "$RESULT_FILE" || echo "NO_STATS" >> "$RESULT_FILE"

echo "=== PROXY LOG ===" >> "$RESULT_FILE"
cat /tmp/proxy_debug.log >> "$RESULT_FILE"

kill $PROXY_PID $MOCK_PID 2>/dev/null
echo "=== TEST DONE ===" >> "$RESULT_FILE"
