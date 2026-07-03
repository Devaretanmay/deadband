use bytes::Bytes;
use deadband_core::{Intervention, Orchestrator};
use std::collections::VecDeque;

#[derive(Clone, Debug, PartialEq)]
pub enum ProcessorState {
    Buffering,
    Analyzing,
    Replaying { index: usize, total: usize },
    Injecting { frame_index: usize },
    Streaming,
    Done,
}

pub struct SseProcessor {
    buffer: VecDeque<Bytes>,
    buffer_size: usize,
    state: ProcessorState,
    intervention: Option<Intervention>,
    injection_frames: Vec<Bytes>,
}

fn make_intervention_sse_openai(prompt: &str) -> Bytes {
    let payload = serde_json::json!({
        "choices": [{
            "index": 0,
            "delta": {
                "role": "assistant",
                "content": format!("\n\n[INTERVENTION] {}\n\nNote: Runtime intervention from Deadband Proxy. Your previous tool call was detected as part of a loop.", prompt)
            }
        }]
    });
    Bytes::from(format!("data: {}\n\n", payload.to_string()))
}

impl SseProcessor {
    pub fn new(buffer_size: usize) -> Self {
        Self {
            buffer: VecDeque::with_capacity(buffer_size),
            buffer_size,
            state: ProcessorState::Buffering,
            intervention: None,
            injection_frames: Vec::new(),
        }
    }

    pub fn push_chunk(
        &mut self,
        chunk: Bytes,
        orchestrator: Option<&mut Orchestrator>,
        thread_id: &str,
    ) -> Option<Bytes> {
        match self.state {
            ProcessorState::Buffering => {
                // Forward the chunk immediately so the client doesn't stall,
                // but also buffer it for replay (if intervention found later).
                self.buffer.push_back(chunk.clone());

                // Scan every chunk for tool calls as it arrives — don't wait
                // for the buffer to fill, which may never happen (flush case)
                // or may miss tool calls that arrive after content deltas.
                if let Some(orch) = orchestrator {
                    self.scan_chunk_for_tool_calls(&chunk, orch, thread_id);
                }

                if self.buffer.len() >= self.buffer_size {
                    // Buffer is full — set up replay/injection state.
                    // Tool calls were already processed per-chunk above,
                    // so analyze() only needs to configure buffer replay.
                    self.state = ProcessorState::Analyzing;
                    self.analyze_for_replay();

                    if !self.injection_frames.is_empty() {
                        // Intervention found — replay buffer with injection frames
                        let total = self.buffer.len();
                        let first = self.buffer[0].clone();
                        self.state = ProcessorState::Replaying { index: 1, total };
                        return Some(first);
                    }

                    // No intervention — skip straight to streaming
                    self.state = ProcessorState::Streaming;
                }
                // Always forward the chunk to the client immediately
                Some(chunk)
            }
            ProcessorState::Analyzing => {
                self.state = ProcessorState::Streaming;
                Some(chunk)
            }
            ProcessorState::Replaying { index, total } => {
                if index < total {
                    let c = self.buffer[index].clone();
                    let new_index = index + 1;
                    let done = new_index >= total;
                    self.state = if done && !self.injection_frames.is_empty() {
                        ProcessorState::Injecting { frame_index: 0 }
                    } else if done {
                        ProcessorState::Streaming
                    } else {
                        ProcessorState::Replaying { index: new_index, total }
                    };
                    Some(c)
                } else {
                    self.state = ProcessorState::Streaming;
                    Some(chunk)
                }
            }
            ProcessorState::Injecting { frame_index } => {
                let next = frame_index + 1;
                if next < self.injection_frames.len() {
                    self.state = ProcessorState::Injecting { frame_index: next };
                    Some(self.injection_frames[frame_index].clone())
                } else if next == self.injection_frames.len() {
                    // Last injection frame emitted — clear so flush()
                    // doesn't re-emit frames that were already sent.
                    let last = self.injection_frames[frame_index].clone();
                    self.injection_frames.clear();
                    self.state = ProcessorState::Streaming;
                    Some(last)
                } else {
                    self.injection_frames.clear();
                    self.state = ProcessorState::Streaming;
                    Some(chunk)
                }
            }
            ProcessorState::Streaming => {
                // Continuously scan for tool calls even after the buffering phase.
                // Tool call SSE deltas often arrive after several content-delta chunks,
                // so waiting for buffer to fill would miss them entirely.
                if let Some(orch) = orchestrator {
                    self.scan_chunk_for_tool_calls(&chunk, orch, thread_id);
                }
                // If scan_chunk_for_tool_calls generated injection frames, emit them
                // immediately rather than leaving them unsent (the client already got
                // the original chunks, so we inject the intervention now).
                if !self.injection_frames.is_empty() {
                    let frame = self.injection_frames[0].clone();
                    self.state = ProcessorState::Injecting { frame_index: 1 };
                    return Some(frame);
                }
                Some(chunk)
            }
            ProcessorState::Done => None,
        }
    }

    /// Configure the processor for replay mode after the buffer has filled
    /// (or on flush). Tool call extraction and orchestrator processing is done
    /// per-chunk in `push_chunk` (both Buffering and Streaming states), so
    /// this only transitions state and returns the first buffer chunk for
    /// replay — it does NOT re-call the orchestrator.
    fn analyze_for_replay(&mut self) -> Option<Bytes> {
        let total = self.buffer.len();
        if total > 0 {
            let first = self.buffer[0].clone();
            self.state = ProcessorState::Replaying { index: 1, total };
            Some(first)
        } else {
            self.state = ProcessorState::Streaming;
            None
        }
    }

    /// Scan a single SSE chunk for tool call deltas and forward them to the
    /// orchestrator. This is called from the `Streaming` state to catch tool
    /// calls that arrive after the initial buffer fills with content chunks.
    fn scan_chunk_for_tool_calls(
        &mut self,
        chunk: &Bytes,
        orchestrator: &mut Orchestrator,
        thread_id: &str,
    ) {
        let text = String::from_utf8_lossy(chunk);
        for line in text.lines() {
            if let Some(data) = line.strip_prefix("data: ") {
                if data == "[DONE]" {
                    continue;
                }
                if let Ok(val) = serde_json::from_str::<serde_json::Value>(data) {
                    self.process_tool_call_json(&val, orchestrator, thread_id);
                }
            }
        }
    }

    /// Common logic to extract tool calls from a parsed SSE JSON value and
    /// forward them to the orchestrator. Handles both OpenAI and Anthropic
    /// streaming formats.
    fn process_tool_call_json(
        &mut self,
        val: &serde_json::Value,
        orchestrator: &mut Orchestrator,
        thread_id: &str,
    ) {
        // OpenAI format: choices[].delta.tool_calls[].function
        if let Some(choices) = val.get("choices").and_then(|c| c.as_array()) {
            for choice in choices {
                if let Some(delta) = choice.get("delta") {
                    if let Some(tool_calls) = delta.get("tool_calls").and_then(|t| t.as_array()) {
                        for tc in tool_calls {
                            if let Some(func) = tc.get("function") {
                                let name = func.get("name").and_then(|n| n.as_str()).unwrap_or("");
                                if !name.is_empty() {
                                    let args = func.get("arguments").and_then(|a| a.as_str()).unwrap_or("");
                                    let event = deadband_core::ToolCallEvent::started(
                                        thread_id,
                                        0,
                                        name,
                                        serde_json::from_str(args).unwrap_or_default(),
                                    );
                                    let (intervention, _report) = orchestrator.process(
                                        event,
                                        &deadband_core::AdapterCapabilities {
                                            retry: true,
                                            inject_prompt: true,
                                            abort: true,
                                            ..Default::default()
                                        },
                                    );
                                    if let Some(intv) = intervention {
                                        if let Some(prompt) = intv.prompt_content() {
                                            self.injection_frames
                                                .push(make_intervention_sse_openai(prompt));
                                        }
                                        self.intervention = Some(intv);
                                        return;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        // Anthropic format: content[].type == "tool_use"
        if let Some(content) = val.get("content").and_then(|c| c.as_array()) {
            for block in content {
                if block.get("type").and_then(|t| t.as_str()) == Some("tool_use") {
                    let name = block.get("name").and_then(|n| n.as_str()).unwrap_or("");
                    if !name.is_empty() {
                        let input = block.get("input").cloned().unwrap_or_default();
                        let event = deadband_core::ToolCallEvent::started(
                            thread_id,
                            0,
                            name,
                            input,
                        );
                        let (intervention, _report) = orchestrator.process(
                            event,
                            &deadband_core::AdapterCapabilities {
                                retry: true,
                                inject_prompt: true,
                                abort: true,
                                ..Default::default()
                            },
                        );
                        if let Some(intv) = intervention {
                            if let Some(prompt) = intv.prompt_content() {
                                self.injection_frames
                                    .push(make_intervention_sse_openai(prompt));
                            }
                            self.intervention = Some(intv);
                            return;
                        }
                    }
                }
            }
        }
    }

    fn extract_tool_calls_from_buffer(&self) -> Vec<(String, String)> {
        let mut calls = Vec::new();
        for chunk in &self.buffer {
            let text = String::from_utf8_lossy(chunk);
            for line in text.lines() {
                if let Some(data) = line.strip_prefix("data: ") {
                    if data == "[DONE]" {
                        continue;
                    }
                    if let Ok(val) = serde_json::from_str::<serde_json::Value>(data) {
                        if let Some(choices) = val.get("choices").and_then(|c| c.as_array()) {
                            for choice in choices {
                                if let Some(delta) = choice.get("delta") {
                                    if let Some(tool_calls) = delta.get("tool_calls").and_then(|t| t.as_array()) {
                                        for tc in tool_calls {
                                            if let Some(func) = tc.get("function") {
                                                let name = func.get("name").and_then(|n| n.as_str()).unwrap_or("");
                                                let args = func.get("arguments").and_then(|a| a.as_str()).unwrap_or("");
                                                if !name.is_empty() {
                                                    calls.push((name.to_string(), args.to_string()));
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        if let Some(content) = val.get("content").and_then(|c| c.as_array()) {
                            for block in content {
                                if block.get("type").and_then(|t| t.as_str()) == Some("tool_use") {
                                    let name = block.get("name").and_then(|n| n.as_str()).unwrap_or("");
                                    let input = block.get("input").map(|i| i.to_string()).unwrap_or_default();
                                    if !name.is_empty() {
                                        calls.push((name.to_string(), input));
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        calls
    }

    pub fn intervention(&self) -> Option<&Intervention> {
        self.intervention.as_ref()
    }

    pub fn has_intervention(&self) -> bool {
        self.intervention.is_some()
    }

    pub fn reset(&mut self) {
        self.buffer.clear();
        self.state = ProcessorState::Buffering;
        self.intervention = None;
        self.injection_frames.clear();
    }

    pub fn state(&self) -> &ProcessorState {
        &self.state
    }

    pub fn flush(&mut self, orchestrator: Option<&mut Orchestrator>, thread_id: &str) -> Option<Bytes> {
        match self.state {
            ProcessorState::Buffering => {
                if self.buffer.is_empty() {
                    self.state = ProcessorState::Done;
                    return None;
                }
                self.state = ProcessorState::Analyzing;
                self.analyze_for_replay();
                // Chunks were already forwarded during buffering (and tool calls
                // were already processed per-chunk), so only replay if there's
                // an intervention with injection frames.
                if !self.injection_frames.is_empty() {
                    let total = self.buffer.len();
                    let first = self.buffer[0].clone();
                    self.state = ProcessorState::Replaying { index: 1, total };
                    return Some(first);
                }
                self.state = ProcessorState::Done;
                None
            }
            ProcessorState::Replaying { index, total } => {
                if index < total {
                    let c = self.buffer[index].clone();
                    self.state = ProcessorState::Replaying { index: index + 1, total };
                    Some(c)
                } else if !self.injection_frames.is_empty() {
                    self.state = ProcessorState::Injecting { frame_index: 0 };
                    self.emit_next_injection()
                } else {
                    self.state = ProcessorState::Done;
                    None
                }
            }
            ProcessorState::Injecting { frame_index } => {
                self.emit_next_injection()
            }
            ProcessorState::Analyzing => {
                self.state = ProcessorState::Streaming;
                None
            }
            ProcessorState::Streaming => {
                // If injection frames were generated during streaming (tool call
                // detected after buffer fill), emit them as a safety net.
                if !self.injection_frames.is_empty() {
                    self.state = ProcessorState::Injecting { frame_index: 0 };
                    return self.emit_next_injection();
                }
                self.state = ProcessorState::Done;
                None
            }
            ProcessorState::Done => None,
        }
    }

    fn emit_next_injection(&mut self) -> Option<Bytes> {
        if let ProcessorState::Injecting { frame_index } = self.state {
            let next = frame_index + 1;
            if next < self.injection_frames.len() {
                let frame = self.injection_frames[frame_index].clone();
                self.state = ProcessorState::Injecting { frame_index: next };
                Some(frame)
            } else if next == self.injection_frames.len() {
                let frame = self.injection_frames[frame_index].clone();
                self.injection_frames.clear();
                self.state = ProcessorState::Done;
                Some(frame)
            } else {
                self.injection_frames.clear();
                self.state = ProcessorState::Done;
                None
            }
        } else {
            self.state = ProcessorState::Done;
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_buffer_initial_chunks_forwarded_immediately() {
        let mut proc = SseProcessor::new(5);
        assert_eq!(proc.state(), &ProcessorState::Buffering);

        let chunk = Bytes::from("data: {\"choices\": [{\"delta\": {}}]}\n\n");
        // Chunks are now forwarded immediately even during buffering
        let result = proc.push_chunk(chunk, None, "test");
        assert!(result.is_some());
        assert_eq!(proc.state(), &ProcessorState::Buffering);
    }

    #[test]
    fn test_buffer_full_triggers_streaming() {
        let mut proc = SseProcessor::new(2);
        let chunk = Bytes::from("data: {}\n\n");

        // Both chunks forwarded immediately
        assert!(proc.push_chunk(chunk.clone(), None, "test").is_some());
        let result = proc.push_chunk(chunk.clone(), None, "test");
        assert!(result.is_some());
        assert_eq!(proc.state(), &ProcessorState::Streaming);
    }

    #[test]
    fn test_extract_tool_calls_openai() {
        let mut proc = SseProcessor::new(10);
        let sse = "data: {\"choices\": [{\"delta\": {\"tool_calls\": [{\"index\": 0, \"function\": {\"name\": \"search\", \"arguments\": \"{\\\"q\\\": \\\"hello\\\"}\"}}]}}]}\n\n";
        let chunk = Bytes::from(sse);
        proc.push_chunk(chunk, None, "test");

        let calls = proc.extract_tool_calls_from_buffer();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].0, "search");
    }

    #[test]
    fn test_injection_frames_generated_for_intervention() {
        let mut proc = SseProcessor::new(1);
        let sse = "data: {\"choices\": [{\"delta\": {\"tool_calls\": [{\"index\": 0, \"function\": {\"name\": \"search\", \"arguments\": \"{\\\"q\\\": \\\"test\\\"}\"}}]}}]}\n\n";
        let chunk = Bytes::from(sse);
        let result = proc.push_chunk(chunk, None, "test");
        assert!(result.is_some());
        let result_bytes = result.unwrap();
        let text = String::from_utf8_lossy(&result_bytes);
        assert!(text.starts_with("data: "));
    }

    #[test]
    fn test_injecting_state_emits_frames() {
        let mut proc = SseProcessor::new(1);
        proc.injection_frames.push(Bytes::from("data: test intervention\n\n"));
        proc.state = ProcessorState::Injecting { frame_index: 0 };

        let chunk = Bytes::from("data: {}\n\n");
        let result = proc.push_chunk(chunk, None, "test");
        assert!(result.is_some());
        let r = result.unwrap();
        assert_eq!(
            String::from_utf8_lossy(&r),
            "data: test intervention\n\n"
        );
        assert_eq!(proc.state(), &ProcessorState::Streaming);
    }

    #[test]
    fn test_reset() {
        let mut proc = SseProcessor::new(3);
        let chunk = Bytes::from("data: {}\n\n");
        proc.push_chunk(chunk.clone(), None, "test");
        proc.push_chunk(chunk.clone(), None, "test");
        proc.push_chunk(chunk, None, "test");

        proc.reset();
        assert_eq!(proc.state(), &ProcessorState::Buffering);
        assert!(proc.intervention().is_none());
        assert!(proc.injection_frames.is_empty());
        assert_eq!(proc.buffer.len(), 0);
    }

    #[test]
    fn test_make_intervention_sse_openai() {
        let result = make_intervention_sse_openai("Stop looping");
        let text = String::from_utf8_lossy(&result);
        assert!(text.starts_with("data: "));
        assert!(text.contains("Stop looping"));
        assert!(text.contains("[INTERVENTION]"));
    }
}
