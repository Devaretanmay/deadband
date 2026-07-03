// HTTP Proxy for Deadband
// Phase 1: Proxy with loop detection

use crate::detector::{LoopDetector, ToolCall};
use crate::intervention::{Intervention, inject_prompt_default};
use crate::stats::Stats;
use anyhow::{Context, Result};
use bytes::Bytes;
use http_body_util::{BodyExt, Full};
use hyper::body::Incoming;
use hyper::service::service_fn;
use hyper::{Request, Response, StatusCode};
use hyper_util::rt::TokioIo;
use serde_json::{Value, json};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tokio::net::TcpListener;

const UPSTREAM_URL: &str = "https://api.openai.com";

/// Shared state for the proxy
pub struct ProxyState {
    pub port: u16,
    pub detector: LoopDetector,
    pub stats: Stats,
    pub data_dir: PathBuf,
}

/// Run the HTTP proxy server
pub async fn run_proxy(state: Arc<Mutex<ProxyState>>) -> Result<()> {
    let port = { state.lock().unwrap().port };
    let addr = format!("0.0.0.0:{}", port);
    let data_dir = { state.lock().unwrap().data_dir.clone() };

    let listener = TcpListener::bind(&addr)
        .await
        .with_context(|| format!("Failed to bind to {}", addr))?;

    tracing::info!("Deadband Proxy listening on {}", addr);
    println!(" Deadband Proxy — Running on port {}", port);
    println!("=====================================");
    println!(" Configure your agent to use:");
    println!("   export OPENAI_BASE_URL=http://localhost:4399/v1");
    println!("\n Commands:");
    println!("   deadband status  — Show loop stats");
    println!("   deadband disable — Stop proxy");

    // Save running status
    {
        let mut s = state.lock().unwrap();
        s.stats.status = "running".to_string();
        s.stats.save(&data_dir.join("stats.json")).ok();
    }

    loop {
        let (stream, peer) = listener.accept().await?;
        let state_clone = state.clone();

        tokio::spawn(async move {
            tracing::debug!("Connection from {}", peer);
            if let Err(e) = handle_connection(stream, state_clone).await {
                tracing::warn!("Connection error from {}: {}", peer, e);
            }
        });
    }
}

async fn handle_connection(
    stream: tokio::net::TcpStream,
    state: Arc<Mutex<ProxyState>>,
) -> Result<()> {
    let io = TokioIo::new(stream);
    let state_clone = state.clone();

    let svc = service_fn(move |req: Request<Incoming>| {
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

/// Parse tool calls from OpenAI chat completion request body
fn parse_tool_calls_from_request(body: &Value) -> Vec<ToolCall> {
    let mut calls = Vec::new();
    
    // Format 1: messages[].tool_calls[] (agent sending tool calls)
    if let Some(messages) = body.get("messages").and_then(|m| m.as_array()) {
        for msg in messages {
            if let Some(tool_calls) = msg.get("tool_calls").and_then(|t| t.as_array()) {
                for tc in tool_calls {
                    if let Some(func) = tc.get("function") {
                        let name = func.get("name").and_then(|n| n.as_str());
                        let args = func.get("arguments").cloned();
                        if let (Some(name), Some(args)) = (name, args) {
                            calls.push(ToolCall {
                                tool_name: name.to_string(),
                                arguments: args,
                            });
                        }
                    }
                }
            }
        }
    }
    
    // Format 2: tools[] at top level (tools available to model)
    // These are tool definitions, not calls - but we can still track them
    if let Some(tools) = body.get("tools").and_then(|t| t.as_array()) {
        for tool in tools {
            // Try both "function" object and direct function fields
            if let Some(func) = tool.get("function") {
                let name = func.get("name").and_then(|n| n.as_str());
                let args = func.get("arguments").cloned();
                if let (Some(name), Some(args)) = (name, args) {
                    calls.push(ToolCall {
                        tool_name: name.to_string(),
                        arguments: args,
                    });
                }
            }
        }
    }
    
    calls
}

/// Create an error response
fn create_error_response(status: StatusCode, message: &str) -> Response<Full<Bytes>> {
    let error_body = json!({
        "error": {
            "message": message,
            "type": "BadRequestError",
            "param": null,
            "code": null
        }
    });
    
    Response::builder()
        .status(status)
        .header("content-type", "application/json")
        .body(Full::from(Bytes::from(error_body.to_string())))
        .unwrap()
}

/// Create an intervention response with injected prompt (SSE chunk format)
fn create_intervention_response(prompt: &str) -> Response<Full<Bytes>> {
    let response_body = json!({
        "id": "chatcmpl-deadband",
        "object": "chat.completion.chunk",
        "created": 0,
        "model": "gpt-4",
        "choices": [{
            "index": 0,
            "delta": {
                "content": prompt
            },
            "finish_reason": null
        }]
    });
    
    Response::builder()
        .status(StatusCode::OK)
        .header("content-type", "application/json")
        .body(Full::from(Bytes::from(response_body.to_string())))
        .unwrap()
}

async fn handle_request(
    req: Request<Incoming>,
    state: Arc<Mutex<ProxyState>>,
) -> Result<Response<Full<Bytes>>, std::convert::Infallible> {
    let path = req.uri().path();
    let method = req.method().clone();

    // Only intercept POST requests to chat completions
    if method != hyper::Method::POST || !path.contains("/v1/chat/completions") {
        return forward_request(req, state).await;
    }

    // Collect request body
    let (parts, body_incoming) = req.into_parts();
    let body_bytes = match body_incoming.collect().await {
        Ok(collected) => collected.to_bytes(),
        Err(e) => {
            tracing::warn!("Failed to read body: {}", e);
            return Ok(create_error_response(
                StatusCode::BAD_REQUEST,
                &format!("Failed to read request: {}", e)
            ));
        }
    };

    // Update request count stats
    {
        let mut s = state.lock().unwrap();
        s.stats.record_request();
    }

    // Parse the request body
    let body_json: Value = match serde_json::from_slice(&body_bytes) {
        Ok(json) => json,
        Err(e) => {
            tracing::warn!("Failed to parse JSON: {}", e);
            return Ok(create_error_response(
                StatusCode::BAD_REQUEST,
                "Invalid JSON in request"
            ));
        }
    };

    // Extract tool calls from request
    let tool_calls = parse_tool_calls_from_request(&body_json);

    // Check each tool call for loops
    let mut intervention: Option<Intervention> = None;
    
    {
        let mut state_lock = state.lock().unwrap();
        
        for call in &tool_calls {
            let (is_loop, _) = state_lock.detector.check(
                call.tool_name.clone(),
                call.arguments.clone(),
            );
            
            if is_loop && intervention.is_none() {
                intervention = Some(inject_prompt_default());
                state_lock.stats.record_loop();
                state_lock.stats.record_intervention();
                tracing::info!("Loop detected for tool: {}", call.tool_name);
            }
        }
        
        state_lock.stats.save(&state_lock.data_dir.join("stats.json")).ok();
    }

    // If loop detected, return intervention
    if let Some(interv) = intervention {
        if interv.is_inject_prompt() {
            return Ok(create_intervention_response(
                interv.content().unwrap_or("Loop detected")
            ));
        } else if interv.is_block() {
            return Ok(create_error_response(
                StatusCode::BAD_REQUEST,
                interv.reason().unwrap_or("Blocked")
            ));
        }
    }

    // No loop, forward to upstream with the original parts and body
    forward_request_with_body(parts, body_bytes, state).await
}

async fn forward_request(
    req: Request<Incoming>,
    _state: Arc<Mutex<ProxyState>>,
) -> Result<Response<Full<Bytes>>, std::convert::Infallible> {
    let client = reqwest::Client::new();
    let upstream_url = UPSTREAM_URL;
    
    let method = match *req.method() {
        hyper::Method::GET => reqwest::Method::GET,
        hyper::Method::POST => reqwest::Method::POST,
        hyper::Method::PUT => reqwest::Method::PUT,
        hyper::Method::DELETE => reqwest::Method::DELETE,
        _ => reqwest::Method::POST,
    };
    
    let uri = format!("{}{}", upstream_url, req.uri());
    let mut builder = client.request(method, &uri);
    
    // Copy headers
    for (key, value) in req.headers() {
        let header_name = key.to_string();
        if let Ok(header_value) = value.to_str() {
            builder = builder.header(header_name, header_value);
        }
    }
    
    // Forward body
    let body_bytes = match req.into_body().collect().await {
        Ok(collected) => collected.to_bytes(),
        Err(_) => Bytes::new(),
    };
    
    let response = match builder.body(body_bytes.to_vec()).send().await {
        Ok(resp) => resp,
        Err(e) => {
            tracing::warn!("Upstream error: {}", e);
            return Ok(create_error_response(
                StatusCode::BAD_REQUEST,
                &format!("Upstream error: {}", e)
            ));
        }
    };
    
    let status = response.status();
    let headers = response.headers().clone();
    let body = response.bytes().await.unwrap_or_default();
    
    let mut response_builder = Response::builder().status(status.as_u16());
    
    for (key, value) in headers.iter() {
        let key_str = key.to_string();
        if let Ok(value_str) = value.to_str() {
            response_builder = response_builder.header(key_str, value_str);
        }
    }
    
    Ok(response_builder.body(Full::from(body.to_vec())).unwrap())
}

async fn forward_request_with_body(
    parts: hyper::http::request::Parts,
    body_bytes: Bytes,
    _state: Arc<Mutex<ProxyState>>,
) -> Result<Response<Full<Bytes>>, std::convert::Infallible> {
    let client = reqwest::Client::new();
    let upstream_url = UPSTREAM_URL;
    
    let method = match parts.method {
        hyper::Method::GET => reqwest::Method::GET,
        hyper::Method::POST => reqwest::Method::POST,
        hyper::Method::PUT => reqwest::Method::PUT,
        hyper::Method::DELETE => reqwest::Method::DELETE,
        _ => reqwest::Method::POST,
    };
    
    let uri = format!("{}{}", upstream_url, parts.uri);
    let mut builder = client.request(method, &uri);
    
    // Copy headers
    for (key, value) in &parts.headers {
        let header_name = key.to_string();
        if let Ok(header_value) = value.to_str() {
            builder = builder.header(header_name, header_value);
        }
    }
    
    let response = match builder.body(body_bytes.to_vec()).send().await {
        Ok(resp) => resp,
        Err(e) => {
            tracing::warn!("Upstream error: {}", e);
            return Ok(create_error_response(
                StatusCode::BAD_REQUEST,
                &format!("Upstream error: {}", e)
            ));
        }
    };
    
    let status = response.status();
    let headers = response.headers().clone();
    let body = response.bytes().await.unwrap_or_default();
    
    let mut response_builder = Response::builder().status(status.as_u16());
    
    for (key, value) in headers.iter() {
        let key_str = key.to_string();
        if let Ok(value_str) = value.to_str() {
            response_builder = response_builder.header(key_str, value_str);
        }
    }
    
    Ok(response_builder.body(Full::from(body.to_vec())).unwrap())
}
