use bytes::Bytes;
use deadband_core::{Intervention, Orchestrator};
use std::collections::VecDeque;

#[derive(Clone, Debug, PartialEq)]
pub enum ProcessorState {
    /// Buffering initial chunks before detection
    Buffering,
    /// Running detection on buffered chunks
    Analyzing,
    /// Replaying buffered chunks (possibly modified)
    Replaying { index: usize, total: usize },
    /// Streaming remaining chunks normally
    Streaming,
    /// Done processing
    Done,
}

pub struct SseProcessor {
    /// Buffered SSE chunks (raw bytes)
    buffer: VecDeque<Bytes>,
    /// Maximum number of chunks to buffer before detection
    buffer_size: usize,
    /// Current processor state
    state: ProcessorState,
    /// Intervention to apply (set after analysis)
    intervention: Option<Intervention>,
    /// Whether to inject a system message mid-stream
    inject_system_message: bool,
}

impl SseProcessor {
    pub fn new(buffer_size: usize) -> Self {
        Self {
            buffer: VecDeque::with_capacity(buffer_size),
            buffer_size,
            state: ProcessorState::Buffering,
            intervention: None,
            inject_system_message: false,
        }
    }

    /// Push a new SSE chunk into the processor.
    /// Returns the chunk to emit (possibly modified), or None if buffering.
    pub fn push_chunk(
        &mut self,
        chunk: Bytes,
        orchestrator: Option<&mut Orchestrator>,
        thread_id: &str,
    ) -> Option<Bytes> {
        match self.state {
            ProcessorState::Buffering => {
                if self.buffer.len() < self.buffer_size {
                    self.buffer.push_back(chunk);
                    if self.buffer.len() >= self.buffer_size {
                        self.state = ProcessorState::Analyzing;
                        return self.analyze(orchestrator, thread_id);
                    }
                    None
                } else {
                    self.state = ProcessorState::Analyzing;
                    self.analyze(orchestrator, thread_id)
                }
            }
            ProcessorState::Analyzing => {
                // Should have transitioned via analyze()
                self.state = ProcessorState::Streaming;
                Some(chunk)
            }
            ProcessorState::Replaying { index, total } => {
                if index < total {
                    let c = self.buffer[index].clone();
                    let new_index = index + 1;
                    self.state = if new_index >= total {
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
            ProcessorState::Streaming => {
                if self.inject_system_message {
                    self.inject_system_message = false;
                    // Don't inject mid-stream for now; handled upstream
                }
                Some(chunk)
            }
            ProcessorState::Done => None,
        }
    }

    /// Analyze buffered chunks to detect loops.
    fn analyze(
        &mut self,
        orchestrator: Option<&mut Orchestrator>,
        thread_id: &str,
    ) -> Option<Bytes> {
        // Collect tool call info from buffered chunks
        let tool_calls = self.extract_tool_calls_from_buffer();

        if let Some(orch) = orchestrator {
            for (tool_name, arguments) in &tool_calls {
                let event = deadband_core::ToolCallEvent::started(
                    thread_id,
                    0,
                    tool_name,
                    serde_json::from_str(arguments).unwrap_or_default(),
                );
                let (intervention, _report) = orch.process(
                    event,
                    &deadband_core::AdapterCapabilities {
                        retry: true,
                        inject_prompt: true,
                        abort: true,
                        ..Default::default()
                    },
                );

                if let Some(intv) = intervention {
                    self.intervention = Some(intv);
                    break;
                }
            }
        }

        // Start replaying buffered chunks
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

    /// Extract tool call data from buffered SSE chunks.
    fn extract_tool_calls_from_buffer(&self) -> Vec<(String, String)> {
        let mut calls = Vec::new();
        for chunk in &self.buffer {
            let text = String::from_utf8_lossy(chunk);
            // Parse SSE lines for tool call deltas
            for line in text.lines() {
                if let Some(data) = line.strip_prefix("data: ") {
                    if data == "[DONE]" {
                        continue;
                    }
                    if let Ok(val) = serde_json::from_str::<serde_json::Value>(data) {
                        // OpenAI format
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
                        // Anthropic format
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

    /// Get the current intervention (if any was detected).
    pub fn intervention(&self) -> Option<&Intervention> {
        self.intervention.as_ref()
    }

    /// Whether a loop was detected in this stream.
    pub fn has_intervention(&self) -> bool {
        self.intervention.is_some()
    }

    /// Reset the processor for a new stream.
    pub fn reset(&mut self) {
        self.buffer.clear();
        self.state = ProcessorState::Buffering;
        self.intervention = None;
        self.inject_system_message = false;
    }

    /// Current state of the processor.
    pub fn state(&self) -> &ProcessorState {
        &self.state
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_buffer_initial_chunks() {
        let mut proc = SseProcessor::new(5);
        assert_eq!(proc.state(), &ProcessorState::Buffering);

        let chunk = Bytes::from("data: {\"choices\": [{\"delta\": {}}]}\n\n");
        let result = proc.push_chunk(chunk, None, "test");
        // Should buffer, not emit
        assert!(result.is_none());
        assert_eq!(proc.state(), &ProcessorState::Buffering);
    }

    #[test]
    fn test_buffer_full_triggers_analysis() {
        let mut proc = SseProcessor::new(2);
        let chunk = Bytes::from("data: {}\n\n");

        // First chunk buffers
        assert!(proc.push_chunk(chunk.clone(), None, "test").is_none());

        // Second chunk should trigger analysis and replay
        let result = proc.push_chunk(chunk.clone(), None, "test");
        assert!(result.is_some());
        assert_eq!(result.unwrap(), chunk);
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
    fn test_reset() {
        let mut proc = SseProcessor::new(3);
        let chunk = Bytes::from("data: {}\n\n");
        proc.push_chunk(chunk.clone(), None, "test");
        proc.push_chunk(chunk.clone(), None, "test");
        proc.push_chunk(chunk, None, "test");

        proc.reset();
        assert_eq!(proc.state(), &ProcessorState::Buffering);
        assert!(proc.intervention().is_none());
        assert_eq!(proc.buffer.len(), 0);
    }
}
