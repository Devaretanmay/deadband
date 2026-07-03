use bytes::Bytes;
use deadband_core::*;
use deadband_proxy::sse::ProcessorState;
use deadband_proxy::sse::SseProcessor;

fn recovery_policy() -> &'static str {
    r#"
policies:
  - name: "recover_on_loop"
    when:
      count:
        ExactRepeat: 2
    do:
      type: "InjectPrompt"
      params:
        content: "The previous tool call was part of a loop. I have removed it. Try a different approach."
        position: "ReplaceLast"
"#
}

fn make_orch(yaml: &str) -> Orchestrator {
    Orchestrator::from_yaml(yaml).unwrap()
}

fn make_event(name: &str, args: serde_json::Value) -> ToolCallEvent {
    ToolCallEvent::started("test", 0, name, args)
}

fn make_failed(name: &str, kind: ErrorKind) -> ToolCallEvent {
    ToolCallEvent::failed("test", 0, name, serde_json::json!({}), kind, "error".to_string(), 0)
}

fn sse_chunk(text: &str) -> Bytes {
    Bytes::from(format!("data: {}\n\n", text))
}

#[test]
fn test_recovery_policy_triggers_at_2_repeats() {
    let mut orch = make_orch(recovery_policy());
    let ev = make_event("read_file", serde_json::json!({"path": "test.py"}));
    let caps = AdapterCapabilities { inject_prompt: true, ..Default::default() };

    let (i, _) = orch.process(ev.clone(), &caps);
    assert!(i.is_none(), "1st call: count=1 < 2");

    let (i, _) = orch.process(ev.clone(), &caps);
    assert!(i.is_some(), "2nd call: count=2 >= 2");
    let intv = i.unwrap();
    assert!(intv.is_inject_prompt());
    assert!(intv.prompt_content().unwrap().contains("loop"));
}

#[test]
fn test_error_pattern_detection() {
    let yaml = r#"
policies:
  - name: "retry_on_timeout"
    when:
      count:
        ErrorPattern: 2
    do:
      type: "Retry"
      params:
        delay_ms: 100
"#;
    let mut orch = make_orch(yaml);
    let caps = AdapterCapabilities { retry: true, ..Default::default() };
    let ev = make_failed("api_call", ErrorKind::Timeout);

    assert!(orch.process(ev.clone(), &caps).0.is_none(), "1st: count=1 < 2");
    let (i, _) = orch.process(ev.clone(), &caps);
    assert!(i.is_some(), "2nd: count=2 >= 2");
    let intv = i.unwrap();
    assert!(intv.is_retry());
    assert_eq!(intv.delay_ms(), Some(100));
}

#[test]
fn test_sse_injection_mode() {
    let mut proc = SseProcessor::new(5);
    proc.set_surgery_state(ProcessorState::Streaming, None, vec![], false);

    let chunk = sse_chunk(r#"{"choices":[{"index":0,"delta":{"tool_calls":[{"index":0,"function":{"name":"search","arguments":"{\"q\":\"test\"}"}}]}}]}"#);

    let mut orch = make_orch(recovery_policy());
    let ev = make_event("search", serde_json::json!({"q": "test"}));
    orch.process(ev, &AdapterCapabilities::default());

    let result = proc.push_chunk(chunk, Some(&mut orch), "test");
    assert!(result.is_some(), "chunk should forward in Streaming mode");
}

#[test]
fn test_sse_surgery_splices_tool_call() {
    let mut proc = SseProcessor::new(5);

    proc.push_buffer(sse_chunk(r#"{"choices":[{"index":0,"delta":{"content":"Let me search."}}]}"#));

    let mut orch = make_orch(recovery_policy());
    let ev = make_event("search", serde_json::json!({"q": "test"}));
    orch.process(ev, &AdapterCapabilities::default());

    let chunk = sse_chunk(r#"{"choices":[{"index":0,"delta":{"tool_calls":[{"index":0,"id":"call_1","type":"function","function":{"name":"search","arguments":"{\"q\":\"test\"}"}}]}}]}"#);
    let r1 = proc.push_chunk(chunk, Some(&mut orch), "test");
    assert!(r1.is_some(), "Buffering forwards chunk normally");

    let (splice_at, _) = proc.surgery_state();
    assert!(splice_at.is_some(), "splice_at should be set after detection");

    let r2 = proc.flush(Some(&mut orch), "test");
    assert!(r2.is_some(), "flush should emit replay + replacement");

    match proc.state() {
        ProcessorState::Replaying { .. } => {},
        ProcessorState::Splicing { .. } => {},
        ProcessorState::Streaming => {},
        other => panic!("unexpected state after flush: {:?}", other),
    }
}

#[test]
fn test_surgery_completes_to_streaming() {
    let mut proc = SseProcessor::new(5);

    let frames = deadband_proxy::sse::make_replacement_frames("Try something else.");
    proc.set_surgery_state(
        ProcessorState::Replaying { index: 1, total: 1 },
        Some(0),
        frames,
        false,
    );
    proc.push_buffer(sse_chunk(r#"{"choices":[{"index":0,"delta":{"content":"Hello"}}]}"#));

    assert!(proc.push_chunk(Bytes::new(), None, "test").is_some());
    assert!(proc.push_chunk(Bytes::new(), None, "test").is_some());

    assert_eq!(proc.state(), &ProcessorState::Streaming);

    let r3 = proc.push_chunk(sse_chunk(r#"{"choices":[{"index":0,"delta":{"content":"fresh data"}}]}"#), None, "test");
    assert!(r3.is_some());
    assert!(String::from_utf8_lossy(&r3.unwrap()).contains("fresh data"));
}

#[test]
fn test_replacement_message_format() {
    let frames = deadband_proxy::sse::make_replacement_frames("Test recovery message.");

    assert_eq!(frames.len(), 2);
    let cf = String::from_utf8_lossy(&frames[0]);
    assert!(cf.starts_with("data: "));
    assert!(cf.contains("Test recovery message."));
    assert!(cf.contains("\"role\":\"assistant\""));
    assert!(!cf.contains("[INTERVENTION]"));

    let sf = String::from_utf8_lossy(&frames[1]);
    assert!(sf.contains("\"finish_reason\":\"stop\""));
}

#[test]
fn test_skip_tool_call_remainder_drops_args() {
    let mut proc = SseProcessor::new(5);
    let frames = deadband_proxy::sse::make_replacement_frames("Recovered.");
    proc.set_surgery_state(
        ProcessorState::Splicing { frame_index: 0 },
        Some(0),
        frames,
        true,
    );

    proc.push_chunk(Bytes::new(), None, "test");
    proc.push_chunk(Bytes::new(), None, "test");
    assert_eq!(proc.state(), &ProcessorState::Streaming);

    let arg = sse_chunk(r#"{"choices":[{"index":0,"delta":{"tool_calls":[{"index":0,"function":{"arguments":"{\"q\":"}}]}}]}"#);
    assert!(proc.push_chunk(arg, None, "test").is_none(), "tool call arg dropped");

    let finish = sse_chunk(r#"{"choices":[{"index":0,"delta":{},"finish_reason":"tool_calls"}]}"#);
    assert!(proc.push_chunk(finish, None, "test").is_none(), "finish reason dropped");

    let clean = sse_chunk(r#"{"choices":[{"index":0,"delta":{"content":"Next turn."}}]}"#);
    assert!(proc.push_chunk(clean, None, "test").is_some());
    let (_, skip) = proc.surgery_state();
    assert!(!skip, "flag cleared");
}

#[test]
fn test_splice_truncates_buffer() {
    let mut proc = SseProcessor::new(10);
    proc.push_buffer(sse_chunk("{\"a\":1}"));
    proc.push_buffer(sse_chunk("{\"b\":2}"));

    let mut orch = make_orch(recovery_policy());
    let ev = make_event("search", serde_json::json!({"q": "test"}));
    orch.process(ev, &AdapterCapabilities::default());

    let chunk = sse_chunk(r#"{"choices":[{"index":0,"delta":{"tool_calls":[{"index":0,"function":{"name":"search","arguments":"{\"q\":\"test\"}"}}]}}]}"#);
    let _ = proc.push_chunk(chunk, Some(&mut orch), "test");

    let r = proc.flush(Some(&mut orch), "test");
    assert!(r.is_some(), "flush should emit");

    let (splice_at, skip) = proc.surgery_state();
    assert!(splice_at.is_some(), "splice should be set");
    assert!(skip, "skip remainder should be set");
}
