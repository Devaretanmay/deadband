
use std::sync::Arc;

use anyhow::{Context, Result};
use bytes::Bytes;
use futures_util::StreamExt;
use http_body_util::{BodyExt, Full, StreamBody};
use hyper::body::Frame;
use hyper::service::service_fn;
use hyper_util::rt::TokioIo;
use std::sync::Mutex;

use deadband_core::{Orchestrator, OrchestratorConfig};

use crate::config::ProxyConfig;
use crate::sse::SseProcessor;

pub struct ProxyState {
    pub config: ProxyConfig,
    pub orchestrator: Mutex<Orchestrator>,
    pub stats: Mutex<ProxyStats>,
}

#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct ProxyStats {
    pub total_requests: u64,
    pub loops_detected: u64,
    pub interventions_applied: u64,
    pub calls_prevented: u64,
    pub estimated_savings: f64,
    pub start_time: chrono::DateTime<chrono::Utc>,
    pub status: String,
}

impl ProxyState {
    pub async fn new(mut config: ProxyConfig) -> Result<Self> {
        tokio::fs::create_dir_all(&config.log_dir).await
            .with_context(|| format!("Failed to create log dir: {:?}", config.log_dir))?;
        tokio::fs::create_dir_all(&config.backups_dir).await
            .with_context(|| format!("Failed to create backups dir: {:?}", config.backups_dir))?;


        let upstream_path = crate::config::ProxyConfig::data_dir().join("upstream_url.txt");
        if let Ok(url) = tokio::fs::read_to_string(&upstream_path).await {
            let url = url.trim().to_string();
            if !url.is_empty() && config.openai_base_url.is_none() {

                let base = url.strip_suffix("/v1").unwrap_or(&url).to_string();
                config.openai_base_url = Some(base);
                tracing::info!("Auto-configured upstream from tool discovery: {}", url);
            }
        }

        let mut policy = tokio::fs::read_to_string(&config.policy_path).await
            .with_context(|| format!("Failed to read policy file: {:?}", config.policy_path))?;

        if config.recover {
            policy.push_str(r#"

  - name: "recover_on_loop"
    when:
      count:
        ExactRepeat: 2
    do:
      type: "InjectPrompt"
      params:
        content: "The previous tool call was part of a loop. I have removed it from the conversation. Try a different approach that does not use this tool in the same way."
        position: "ReplaceLast"
"#);
            tracing::info!("Recovery mode enabled — loop detection at 2 repeats triggers context surgery");
        }

        let orchestrator = Orchestrator::new(
            OrchestratorConfig::default(),
            &policy,
            Vec::new(),
        ).map_err(|e| anyhow::anyhow!("Failed to create orchestrator: {}", e))?;

        Ok(Self {
            config,
            orchestrator: Mutex::new(orchestrator),
            stats: Mutex::new(ProxyStats {
                start_time: chrono::Utc::now(),
                status: "starting".to_string(),
                ..Default::default()
            }),
        })
    }




    pub fn record_request(&self, loops: u64, interventions: u64, prevented: u64) {
        {
            let mut stats = self.stats.lock().unwrap();
            stats.total_requests += 1;
            stats.loops_detected += loops;
            stats.interventions_applied += interventions;
            stats.calls_prevented += prevented;
            stats.estimated_savings = stats.calls_prevented as f64 * 0.002;
            stats.status = "running".to_string();
        }
        self.persist_stats();
    }



    fn persist_stats(&self) {
        let data_dir = crate::config::ProxyConfig::data_dir();
        if let Err(e) = std::fs::create_dir_all(&data_dir) {
            tracing::warn!("Failed to create stats directory {:?}: {}", data_dir, e);
            return;
        }
        let stats_path = data_dir.join("stats.json");
        let stats = self.stats.lock().unwrap();
        match serde_json::to_string_pretty(&*stats) {
            Ok(json) => {
                if let Err(e) = std::fs::write(&stats_path, json) {
                    tracing::warn!("Failed to write stats to {:?}: {}", stats_path, e);
                }
            }
            Err(e) => {
                tracing::warn!("Failed to serialize stats: {}", e);
            }
        }
    }
}

type BoxedBody = http_body_util::combinators::BoxBody<Bytes, std::convert::Infallible>;

pub async fn run_proxy(state: Arc<ProxyState>) -> Result<()> {
    let port = state.config.port;
    let actual_port = find_available_port(port).await
        .with_context(|| format!("No available port found starting from {}", port))?;

    let addr = format!("0.0.0.0:{}", actual_port);

    {
        let mut stats = state.stats.lock().unwrap();
        stats.status = "running".to_string();
    }

    tracing::info!("Deadband Proxy starting on {}", addr);

    let listener = tokio::net::TcpListener::bind(&addr).await
        .with_context(|| format!("Failed to bind to {}", addr))?;

    loop {
        let (stream, peer) = listener.accept().await?;
        let state = state.clone();

        tokio::spawn(async move {
            tracing::debug!("Connection from {}", peer);
            if let Err(e) = handle_connection(stream, state).await {
                tracing::warn!("Connection error from {}: {}", peer, e);
            }
        });
    }
}

async fn find_available_port(start: u16) -> Option<u16> {
    for port in start..start.saturating_add(10) {
        match tokio::net::TcpListener::bind(format!("0.0.0.0:{}", port)).await {
            Ok(_) => return Some(port),
            Err(_) => continue,
        }
    }
    None
}

async fn handle_connection(
    stream: tokio::net::TcpStream,
    state: Arc<ProxyState>,
) -> Result<()> {
    let io = TokioIo::new(stream);
    let state_clone = state.clone();

    let svc = service_fn(move |req: hyper::Request<hyper::body::Incoming>| {
        let state = state_clone.clone();
        async move { handle_request(req, state).await }
    });

    let conn = hyper::server::conn::http1::Builder::new()
        .serve_connection(io, svc)
        .with_upgrades();

    if let Err(e) = conn.await {
        tracing::warn!("Connection error: {}", e);
    }

    Ok(())
}

async fn handle_request(
    req: hyper::Request<hyper::body::Incoming>,
    state: Arc<ProxyState>,
) -> Result<hyper::Response<BoxedBody>, std::convert::Infallible> {
    let path = req.uri().path().to_string();
    let method = req.method().clone();

    let headers: Vec<(String, String)> = req.headers().iter().map(|(key, value)| {
        (key.to_string(), value.to_str().unwrap_or("").to_string())
    }).collect();

    let api_key = crate::request::extract_api_key(&headers);

    let body_bytes = match req.collect().await {
        Ok(collected) => collected.to_bytes(),
        Err(e) => {
            return Ok(hyper::Response::builder()
                .status(400)
                .body(Full::from(Bytes::from(format!("Failed to read body: {}", e))).boxed())
                .unwrap());
        }
    };
    let body_str = String::from_utf8_lossy(&body_bytes);

    let parsed = match crate::request::parse_request(&body_str, &path, &headers) {
        Ok(r) => r,
        Err(e) => {
            return Ok(hyper::Response::builder()
                .status(400)
                .body(Full::from(Bytes::from(format!("Parse error: {}", e))).boxed())
                .unwrap());
        }
    };

    let is_streaming = match &parsed {
        crate::request::ApiRequest::OpenAI { stream, .. } => *stream,
        crate::request::ApiRequest::Anthropic { stream, .. } => *stream,
    };

    let upstream_base = determine_upstream_url(&parsed, &state.config);
    let upstream_url = format!("{}{}", upstream_base, path);

    let upstream_body = crate::request::build_upstream_body(&parsed, None);
    let client = reqwest::Client::new();
    let reqwest_method = match method {
        hyper::Method::GET => reqwest::Method::GET,
        hyper::Method::POST => reqwest::Method::POST,
        hyper::Method::PUT => reqwest::Method::PUT,
        hyper::Method::DELETE => reqwest::Method::DELETE,
        _ => reqwest::Method::POST,
    };
    let mut upstream_req = client.request(reqwest_method, &upstream_url)
        .json(&upstream_body);

    if let Some(key) = &api_key {
        if path.starts_with("/v1/messages") {
            upstream_req = upstream_req.header("x-api-key", key);
            upstream_req = upstream_req.header("anthropic-version", "2023-06-01");
        } else {
            upstream_req = upstream_req.header("Authorization", format!("Bearer {}", key));
        }
    }

    let upstream_resp = match upstream_req.send().await {
        Ok(r) => r,
        Err(e) => {
            return Ok(hyper::Response::builder()
                .status(502)
                .body(Full::from(Bytes::from(format!("Upstream error: {}", e))).boxed())
                .unwrap());
        }
    };

    let upstream_status = upstream_resp.status();
    let upstream_headers = upstream_resp.headers().clone();

    if is_streaming {
        handle_streaming_response(upstream_resp, upstream_status, upstream_headers, state).await
    } else {
        handle_non_streaming_response(upstream_resp, upstream_status, upstream_headers, &parsed, state).await
    }
}

fn determine_upstream_url(
    request: &crate::request::ApiRequest,
    config: &ProxyConfig,
) -> String {
    match request {
        crate::request::ApiRequest::OpenAI { .. } => {
            config.openai_base_url.clone().unwrap_or_else(|| "https://api.openai.com".to_string())
        }
        crate::request::ApiRequest::Anthropic { .. } => {
            config.anthropic_base_url.clone().unwrap_or_else(|| "https://api.anthropic.com".to_string())
        }
    }
}

async fn handle_non_streaming_response(
    resp: reqwest::Response,
    status: reqwest::StatusCode,
    headers: reqwest::header::HeaderMap,
    request: &crate::request::ApiRequest,
    state: Arc<ProxyState>,
) -> Result<hyper::Response<BoxedBody>, std::convert::Infallible> {
    let body = resp.bytes().await.unwrap_or_default();

    let intervention_content = if let Ok(body_val) = serde_json::from_slice::<serde_json::Value>(&body) {
        let tool_name = body_val.get("choices")
            .and_then(|c| c.as_array())
            .and_then(|c| c.first())
            .and_then(|c| c.get("message"))
            .and_then(|m| m.get("tool_calls"))
            .and_then(|tc| tc.as_array())
            .and_then(|tc| tc.first())
            .and_then(|tc| tc.get("function"))
            .and_then(|f| f.get("name"))
            .and_then(|n| n.as_str())
            .unwrap_or("");

        if !tool_name.is_empty() {
            let event = deadband_core::ToolCallEvent::started(
                "proxy", 0, tool_name, serde_json::Value::Null,
            );
            let (intervention, _report) = {
                let mut orch = state.orchestrator.lock().unwrap();
                orch.process(event, &deadband_core::AdapterCapabilities {
                    retry: true, inject_prompt: true, abort: true,
                    ..Default::default()
                })
            };
            intervention.and_then(|i| i.prompt_content().map(|c| c.to_string()))
        } else {
            None
        }
    } else {
        None
    };

    let mut builder = hyper::Response::builder().status(status.as_u16());
    for (key, value) in headers.iter() {
        if key != "transfer-encoding" && key != "connection" {
            builder = builder.header(key.as_str(), value.to_str().unwrap_or(""));
        }
    }

    let final_body = if let Some(content) = &intervention_content {
        let modified = crate::request::build_upstream_body(request, Some(content));
        serde_json::to_vec(&modified).unwrap_or_else(|_| body.to_vec())
    } else {
        body.to_vec()
    };

    let had_intervention = intervention_content.is_some();
    if had_intervention {
        state.record_request(1, 1, 1);
        tracing::info!("Non-streaming intervention applied");
    } else {
        state.record_request(0, 0, 0);
    }

    Ok(builder.body(Full::from(Bytes::from(final_body)).boxed()).unwrap())
}

async fn handle_streaming_response(
    resp: reqwest::Response,
    status: reqwest::StatusCode,
    upstream_headers: reqwest::header::HeaderMap,
    state: Arc<ProxyState>,
) -> Result<hyper::Response<BoxedBody>, std::convert::Infallible> {
    let mut builder = hyper::Response::builder().status(status.as_u16());


    for (key, value) in upstream_headers.iter() {
        let key_str = key.as_str();

        if key_str == "transfer-encoding"
            || key_str == "connection"
            || key_str == "content-length"
        {
            continue;
        }
        if let Ok(val) = value.to_str() {
            builder = builder.header(key_str, val);
        }
    }

    builder = builder.header("content-type", "text/event-stream");
    builder = builder.header("cache-control", "no-cache");

    let mut sse_proc = SseProcessor::new(state.config.sse_buffer_size);
    let state_clone = state.clone();

    let (tx, rx) = tokio::sync::mpsc::channel::<Result<Frame<Bytes>, std::convert::Infallible>>(64);

    tokio::spawn(async move {
        let mut stream = resp.bytes_stream();
        let mut has_intervened = false;

        while let Some(chunk_result) = stream.next().await {
            let data = match chunk_result {
                Ok(chunk) => {
                    let mut orch = state_clone.orchestrator.lock().unwrap();
                    let result = sse_proc.push_chunk(chunk, Some(&mut orch), "proxy");

                    if sse_proc.has_intervention() && !has_intervened {
                        has_intervened = true;
                        state_clone.record_request(1, 1, 1);
                        tracing::info!("Streaming intervention applied");
                    }

                    result
                }
                Err(_) => None,
            };

            if let Some(data) = data {
                if tx.send(Ok(Frame::data(data))).await.is_err() {
                    break;
                }
            }
        }


        let (flush_data, had_any_intervention) = {
            let mut orch = state_clone.orchestrator.lock().unwrap();
            let had = sse_proc.has_intervention();
            let mut data = Vec::new();
            while let Some(chunk) = sse_proc.flush(Some(&mut orch), "proxy") {
                data.push(chunk);
            }



            if !had && sse_proc.has_intervention() {
                has_intervened = true;
                state_clone.record_request(1, 1, 1);
                tracing::info!("Streaming intervention applied during flush");
            }
            (data, has_intervened || had)
        };

        for data in flush_data {
            if tx.send(Ok(Frame::data(data))).await.is_err() {
                break;
            }
        }



        if !had_any_intervention {
            state_clone.record_request(0, 0, 0);
        }
    });

    let rx_stream = tokio_stream::wrappers::ReceiverStream::new(rx);
    let body: BoxedBody = BodyExt::boxed(StreamBody::new(rx_stream));

    tracing::debug!("returning streaming response");
    Ok(builder.body(body).unwrap())
}
