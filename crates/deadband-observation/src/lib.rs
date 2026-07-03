
pub mod canonical;
pub mod detection;
pub mod event;
pub mod history;
pub mod pipeline;
pub mod report;

pub use canonical::{auto_infer_volatile_fields, strip_volatile_fields};
pub use detection::{Detection, Detector, DetectorBox};
pub use event::{ErrorKind, Payload, ToolCallEvent};
pub use history::HistoryStore;
pub use pipeline::ObservationPipeline;
pub use report::DetectionReport;

pub fn canonicalize_args(args_json: &str, volatile_fields: &[String]) -> String {
    let mut val: serde_json::Value = serde_json::from_str(args_json).unwrap_or(serde_json::Value::Null);
    let paths: Vec<String> = volatile_fields.iter()
        .map(|f| format!(".{}", f))
        .collect();
    if !paths.is_empty() {
        canonical::strip_volatile_fields(&mut val, &paths);
    }
    serde_json::to_string(&val).unwrap_or_else(|_| args_json.to_string())
}
