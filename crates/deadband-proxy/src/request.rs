use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Debug)]
pub enum ApiRequest {
    OpenAI {
        messages: Vec<Message>,
        tools: Vec<Tool>,
        stream: bool,
        model: String,
        raw: Value,
    },
    Anthropic {
        messages: Vec<Message>,
        tools: Vec<Tool>,
        stream: bool,
        model: String,
        raw: Value,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Message {
    pub role: String,
    pub content: Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCall>>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    #[serde(rename = "type")]
    pub call_type: String,
    pub function: FunctionCall,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FunctionCall {
    pub name: String,
    pub arguments: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Tool {
    #[serde(rename = "type")]
    pub tool_type: String,
    pub function: Option<ToolFunction>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ToolFunction {
    pub name: String,
    pub description: Option<String>,
    pub parameters: Option<Value>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ToolCallDelta {
    pub index: u64,
    pub id: Option<String>,
    #[serde(rename = "type")]
    pub delta_type: Option<String>,
    pub function: Option<FunctionDelta>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FunctionDelta {
    pub name: Option<String>,
    pub arguments: Option<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum RequestError {
    #[error("Unsupported endpoint: {0}")]
    UnsupportedEndpoint(String),
    #[error("JSON parse error: {0}")]
    JsonParse(String),
    #[error("Missing API key")]
    MissingApiKey,
}

pub fn extract_api_key(headers: &[(String, String)]) -> Option<String> {
    for (key, value) in headers {
        if key.eq_ignore_ascii_case("authorization") {
            if let Some(key) = value.strip_prefix("Bearer ") {
                return Some(key.to_string());
            }
            return Some(value.to_string());
        }
        if key.eq_ignore_ascii_case("x-api-key") {
            return Some(value.to_string());
        }
    }
    None
}

pub fn parse_request(body: &str, path: &str, _headers: &[(String, String)]) -> Result<ApiRequest, RequestError> {
    let raw: Value = serde_json::from_str(body).map_err(|e| RequestError::JsonParse(e.to_string()))?;

    let messages = raw.get("messages")
        .and_then(|v| serde_json::from_value::<Vec<Message>>(v.clone()).ok())
        .unwrap_or_default();
    let tools = raw.get("tools")
        .and_then(|v| serde_json::from_value::<Vec<Tool>>(v.clone()).ok())
        .unwrap_or_default();
    let stream = raw.get("stream").and_then(|v| v.as_bool()).unwrap_or(false);
    let model = raw.get("model").and_then(|v| v.as_str()).unwrap_or("unknown").to_string();

    if path.starts_with("/v1/chat/completions") || path.starts_with("/v1/completions") {
        Ok(ApiRequest::OpenAI { messages, tools, stream, model, raw })
    } else if path.starts_with("/v1/messages") {
        Ok(ApiRequest::Anthropic { messages, tools, stream, model, raw })
    } else {
        Err(RequestError::UnsupportedEndpoint(path.to_string()))
    }
}

pub fn build_upstream_body(request: &ApiRequest, intervention_content: Option<&str>) -> Value {
    let strip_provider = |model: &str| -> String {
        model.split_once('/').map(|(_, rest)| rest).unwrap_or(model).to_string()
    };

    match request {
        ApiRequest::OpenAI { model, raw, .. } => {
            let mut body = raw.clone();
            body["model"] = serde_json::Value::String(strip_provider(model));
            if let Some(content) = intervention_content {

                if let Some(messages) = body.get_mut("messages").and_then(|m| m.as_array_mut()) {
                    let system_msg = serde_json::json!({
                        "role": "system",
                        "content": format!("[INTERVENTION] {}\n\nNote: This is a runtime intervention from Deadband Proxy. Your previous tool call was detected as part of a loop.", content)
                    });
                    messages.push(system_msg);
                }
            }
            body
        }
        ApiRequest::Anthropic { model, raw, .. } => {
            let mut body = raw.clone();
            body["model"] = serde_json::Value::String(strip_provider(model));
            if let Some(content) = intervention_content {
                if let Some(messages) = body.get_mut("messages").and_then(|m| m.as_array_mut()) {
                    let system_msg = serde_json::json!({
                        "role": "user",
                        "content": format!("[INTERVENTION] {}\n\nNote: This is a runtime intervention from Deadband Proxy. Your previous tool call was detected as part of a loop.", content)
                    });
                    messages.push(system_msg);
                }
            }
            body
        }
    }
}
