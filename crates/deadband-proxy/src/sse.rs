use bytes::Bytes;
use deadband_core::{Intervention, Orchestrator};
use std::collections::VecDeque;

#[derive(Clone, Debug, PartialEq)]
pub enum ProcessorState {
    Buffering,
    Analyzing,
    Replaying { index: usize, total: usize },
    Injecting { frame_index: usize },
    Splicing { frame_index: usize },
    Streaming,
    Done,
}

pub struct SseProcessor {
    buffer: VecDeque<Bytes>,
    buffer_size: usize,
    state: ProcessorState,
    intervention: Option<Intervention>,
    injection_frames: Vec<Bytes>,
    splice_at: Option<usize>,
    replacement_frames: Vec<Bytes>,
    skip_tool_call_remainder: bool,
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

pub fn make_replacement_frames(prompt: &str) -> Vec<Bytes> {
    let content_payload = serde_json::json!({
        "choices": [{
            "index": 0,
            "delta": {
                "role": "assistant",
                "content": prompt
            }
        }]
    });
    let stop_payload = serde_json::json!({
        "choices": [{
            "index": 0,
            "finish_reason": "stop",
            "delta": {}
        }]
    });
    vec![
        Bytes::from(format!("data: {}\n\n", content_payload.to_string())),
        Bytes::from(format!("data: {}\n\n", stop_payload.to_string())),
    ]
}

impl SseProcessor {
    pub fn new(buffer_size: usize) -> Self {
        Self {
            buffer: VecDeque::with_capacity(buffer_size),
            buffer_size,
            state: ProcessorState::Buffering,
            intervention: None,
            injection_frames: Vec::new(),
            splice_at: None,
            replacement_frames: Vec::new(),
            skip_tool_call_remainder: false,
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


                self.buffer.push_back(chunk.clone());




                if let Some(orch) = orchestrator {
                    self.scan_chunk_for_tool_calls(&chunk, orch, thread_id);
                }

                if self.splice_at.is_some() {
                    if self.buffer.len() > self.splice_at.unwrap() + 1 {
                        self.buffer.truncate(self.splice_at.unwrap() + 1);
                    }
                    self.skip_tool_call_remainder = true;
                    let total = self.splice_at.unwrap();
                    if total > 0 {
                        self.state = ProcessorState::Replaying { index: 1, total };
                        return Some(self.buffer[0].clone());
                    }
                    self.state = ProcessorState::Splicing { frame_index: 0 };
                    return self.emit_next_replacement();
                }

                if self.buffer.len() >= self.buffer_size {
                    self.state = ProcessorState::Analyzing;
                    self.analyze_for_replay();

                    if !self.injection_frames.is_empty() {
                        let total = self.buffer.len();
                        let first = self.buffer[0].clone();
                        self.state = ProcessorState::Replaying { index: 1, total };
                        return Some(first);
                    }

                    self.state = ProcessorState::Streaming;
                }

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
                    self.state = if done {
                        if !self.replacement_frames.is_empty() {
                            ProcessorState::Splicing { frame_index: 0 }
                        } else if !self.injection_frames.is_empty() {
                            ProcessorState::Injecting { frame_index: 0 }
                        } else {
                            ProcessorState::Streaming
                        }
                    } else {
                        ProcessorState::Replaying { index: new_index, total }
                    };
                    Some(c)
                } else {
                    if !self.replacement_frames.is_empty() {
                        self.state = ProcessorState::Splicing { frame_index: 0 };
                        self.emit_next_replacement()
                    } else if !self.injection_frames.is_empty() {
                        self.state = ProcessorState::Injecting { frame_index: 0 };
                        self.emit_next_injection()
                    } else {
                        self.state = ProcessorState::Streaming;
                        Some(chunk)
                    }
                }
            }
            ProcessorState::Injecting { frame_index } => {
                let next = frame_index + 1;
                if next < self.injection_frames.len() {
                    self.state = ProcessorState::Injecting { frame_index: next };
                    Some(self.injection_frames[frame_index].clone())
                } else if next == self.injection_frames.len() {
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
            ProcessorState::Splicing { frame_index } => {
                let next = frame_index + 1;
                if next < self.replacement_frames.len() {
                    self.state = ProcessorState::Splicing { frame_index: next };
                    Some(self.replacement_frames[frame_index].clone())
                } else if next == self.replacement_frames.len() {
                    let last = self.replacement_frames[frame_index].clone();
                    self.replacement_frames.clear();
                    self.state = ProcessorState::Streaming;
                    Some(last)
                } else {
                    self.replacement_frames.clear();
                    self.state = ProcessorState::Streaming;
                    Some(chunk)
                }
            }
            ProcessorState::Streaming => {
                if self.skip_tool_call_remainder {
                    let text = String::from_utf8_lossy(&chunk);
                    if text.contains("\"finish_reason\"")
                        || (text.contains("\"tool_calls\"") && text.contains("\"function\""))
                    {
                        return None;
                    }
                    self.skip_tool_call_remainder = false;
                }

                if let Some(orch) = orchestrator {
                    self.scan_chunk_for_tool_calls(&chunk, orch, thread_id);
                }



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




    fn process_tool_call_json(
        &mut self,
        val: &serde_json::Value,
        orchestrator: &mut Orchestrator,
        thread_id: &str,
    ) {

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
                                            match self.state {
                                                ProcessorState::Buffering => {
                                                    self.splice_at = Some(self.buffer.len() - 1);
                                                    self.replacement_frames.extend(make_replacement_frames(prompt));
                                                }
                                                _ => {
                                                    self.injection_frames.push(make_intervention_sse_openai(prompt));
                                                }
                                            }
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
                                match self.state {
                                    ProcessorState::Buffering => {
                                        self.splice_at = Some(self.buffer.len() - 1);
                                        self.replacement_frames.extend(make_replacement_frames(prompt));
                                    }
                                    _ => {
                                        self.injection_frames.push(make_intervention_sse_openai(prompt));
                                    }
                                }
                            }
                            self.intervention = Some(intv);
                            return;
                        }
                    }
                }
            }
        }
    }

    pub fn intervention(&self) -> Option<&Intervention> {
        self.intervention.as_ref()
    }

    pub fn has_intervention(&self) -> bool {
        self.intervention.is_some()
    }

    pub fn set_surgery_state(
        &mut self,
        state: ProcessorState,
        splice_at: Option<usize>,
        replacement_frames: Vec<Bytes>,
        skip_remainder: bool,
    ) {
        self.state = state;
        self.splice_at = splice_at;
        self.replacement_frames = replacement_frames;
        self.skip_tool_call_remainder = skip_remainder;
    }

    pub fn surgery_state(&self) -> (Option<usize>, bool) {
        (self.splice_at, self.skip_tool_call_remainder)
    }

    pub fn push_buffer(&mut self, chunk: Bytes) {
        self.buffer.push_back(chunk);
    }

    pub fn reset(&mut self) {
        self.buffer.clear();
        self.state = ProcessorState::Buffering;
        self.intervention = None;
        self.injection_frames.clear();
        self.splice_at = None;
        self.replacement_frames.clear();
        self.skip_tool_call_remainder = false;
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

                if self.intervention.is_none() {
                    if let Some(orch) = orchestrator {
                        for i in 0..self.buffer.len() {
                            let chunk = self.buffer[i].clone();
                            self.scan_chunk_for_tool_calls(&chunk, orch, thread_id);

                            if self.intervention.is_some() {
                                break;
                            }
                        }
                    }
                }

                if self.splice_at.is_some() {
                    if self.buffer.len() > self.splice_at.unwrap() + 1 {
                        self.buffer.truncate(self.splice_at.unwrap() + 1);
                    }
                    self.skip_tool_call_remainder = true;
                    let total = self.splice_at.unwrap();
                    if total > 0 {
                        self.state = ProcessorState::Replaying { index: 1, total };
                        return Some(self.buffer[0].clone());
                    }
                    self.state = ProcessorState::Splicing { frame_index: 0 };
                    return self.emit_next_replacement();
                }

                self.state = ProcessorState::Analyzing;
                self.analyze_for_replay();

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
                } else if !self.replacement_frames.is_empty() {
                    self.state = ProcessorState::Splicing { frame_index: 0 };
                    self.emit_next_replacement()
                } else if !self.injection_frames.is_empty() {
                    self.state = ProcessorState::Injecting { frame_index: 0 };
                    self.emit_next_injection()
                } else {
                    self.state = ProcessorState::Done;
                    None
                }
            }
            ProcessorState::Injecting { .. } => {
                self.emit_next_injection()
            }
            ProcessorState::Splicing { .. } => {
                self.emit_next_replacement()
            }
            ProcessorState::Analyzing => {
                self.state = ProcessorState::Streaming;
                None
            }
            ProcessorState::Streaming => {


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

    fn emit_next_replacement(&mut self) -> Option<Bytes> {
        if let ProcessorState::Splicing { frame_index } = self.state {
            let next = frame_index + 1;
            if next < self.replacement_frames.len() {
                let frame = self.replacement_frames[frame_index].clone();
                self.state = ProcessorState::Splicing { frame_index: next };
                Some(frame)
            } else if next == self.replacement_frames.len() {
                let last = self.replacement_frames[frame_index].clone();
                self.replacement_frames.clear();
                self.state = ProcessorState::Done;
                Some(last)
            } else {
                self.replacement_frames.clear();
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

        let result = proc.push_chunk(chunk, None, "test");
        assert!(result.is_some());
        assert_eq!(proc.state(), &ProcessorState::Buffering);
    }

    #[test]
    fn test_buffer_full_triggers_streaming() {
        let mut proc = SseProcessor::new(2);
        let chunk = Bytes::from("data: {}\n\n");


        assert!(proc.push_chunk(chunk.clone(), None, "test").is_some());
        let result = proc.push_chunk(chunk.clone(), None, "test");
        assert!(result.is_some());
        assert_eq!(proc.state(), &ProcessorState::Streaming);
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

    #[test]
    fn test_splicing_removes_tool_call() {
        let mut proc = SseProcessor::new(10);
        proc.buffer.push_back(Bytes::from("data: {\"choices\":[{\"index\":0,\"delta\":{\"content\":\"Hello\"}}]}\n\n"));
        proc.buffer.push_back(Bytes::from("data: {\"choices\":[{\"index\":0,\"delta\":{\"tool_calls\":[{\"index\":0,\"function\":{\"name\":\"search\",\"arguments\":\"{}\"}}]}}]}\n\n"));
        proc.buffer.push_back(Bytes::from("data: {\"choices\":[{\"index\":0,\"delta\":{\"content\":\" after\"}}]}\n\n"));
        proc.splice_at = Some(1);
        proc.replacement_frames = make_replacement_frames("Let me try a different approach.");

        proc.state = ProcessorState::Replaying { index: 1, total: 1 };

        let r1 = proc.push_chunk(Bytes::new(), None, "test");
        assert!(r1.is_some());
        let r1b = r1.unwrap();
        let t1 = String::from_utf8_lossy(&r1b);
        assert!(t1.contains("Let me try a different approach"));
        assert!(!t1.contains("[INTERVENTION]"));

        let r2 = proc.push_chunk(Bytes::new(), None, "test");
        assert!(r2.is_some());
        let r2b = r2.unwrap();
        let t2 = String::from_utf8_lossy(&r2b);
        assert!(t2.contains("finish_reason"));

        assert_eq!(proc.state(), &ProcessorState::Streaming);

        let r3 = proc.push_chunk(Bytes::from("data: {\"choices\":[{\"index\":0,\"delta\":{\"content\":\"passthrough\"}}]}\n\n"), None, "test");
        assert!(r3.is_some());
        assert!(String::from_utf8_lossy(&r3.unwrap()).contains("passthrough"));
    }

    #[test]
    fn test_splicing_then_streaming() {
        let mut proc = SseProcessor::new(10);
        proc.buffer.push_back(Bytes::from("data: {\"choices\":[{\"index\":0,\"delta\":{\"content\":\"Hello\"}}]}\n\n"));
        proc.buffer.push_back(Bytes::from("data: {\"choices\":[{\"index\":0,\"delta\":{\"tool_calls\":[{\"index\":0,\"function\":{\"name\":\"search\",\"arguments\":\"{}\"}}]}}]}\n\n"));
        proc.splice_at = Some(1);
        proc.replacement_frames = make_replacement_frames("Trying something else.");

        proc.state = ProcessorState::Replaying { index: 1, total: 1 };
        let _ = proc.push_chunk(Bytes::new(), None, "test");
        let _ = proc.push_chunk(Bytes::new(), None, "test");

        assert_eq!(proc.state(), &ProcessorState::Streaming);

        let r = proc.push_chunk(Bytes::from("data: {\"choices\":[{\"index\":0,\"delta\":{\"content\":\"new data\"}}]}\n\n"), None, "test");
        assert!(r.is_some());
        assert!(String::from_utf8_lossy(&r.unwrap()).contains("new data"));
    }

    #[test]
    fn test_splicing_no_replay_needed() {
        let mut proc = SseProcessor::new(10);
        proc.buffer.push_back(Bytes::from("data: {\"choices\":[{\"index\":0,\"delta\":{\"tool_calls\":[{\"index\":0,\"function\":{\"name\":\"search\",\"arguments\":\"{}\"}}]}}]}\n\n"));
        proc.buffer.push_back(Bytes::from("data: {\"choices\":[{\"index\":0,\"delta\":{\"content\":\" after\"}}]}\n\n"));
        proc.splice_at = Some(0);
        proc.replacement_frames = make_replacement_frames("Trying something else.");

        proc.state = ProcessorState::Splicing { frame_index: 0 };

        let r = proc.push_chunk(Bytes::new(), None, "test");
        assert!(r.is_some());
        assert!(String::from_utf8_lossy(&r.unwrap()).contains("Trying something else."));

        let r = proc.push_chunk(Bytes::new(), None, "test");
        assert!(r.is_some());

        assert_eq!(proc.state(), &ProcessorState::Streaming);

        let r2 = proc.push_chunk(Bytes::from("data: {\"choices\":[{\"index\":0,\"delta\":{\"content\":\"after\"}}]}\n\n"), None, "test");
        assert!(r2.is_some());
        assert!(String::from_utf8_lossy(&r2.unwrap()).contains("after"));
    }
}
