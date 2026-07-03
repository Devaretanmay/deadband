use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::SystemTime;

use deadband_observation::event::ToolCallEvent;
use serde_json::json;

use crate::proxy::ProxyState;

struct FileEntry {
    len: u64,
    mtime: SystemTime,
}

pub async fn run_watchdog(state: Arc<ProxyState>, watch_dir: PathBuf, step_counter: Arc<std::sync::atomic::AtomicU64>) {
    let mut files: HashMap<PathBuf, FileEntry> = HashMap::new();

    loop {
        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

        let scan = match scan_dir(&watch_dir) {
            Ok(s) => s,
            Err(e) => {
                tracing::warn!("watchdog scan error: {}", e);
                continue;
            }
        };

        for (path, entry) in &scan {
            if let Some(prev) = files.get(path) {
                if prev.len != entry.len || prev.mtime != entry.mtime {
                    let step = step_counter.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    let event = ToolCallEvent::succeeded(
                        "watchdog",
                        step,
                        "watchdog.file_modified",
                        json!({"path": path.to_string_lossy()}),
                        json!({"size": entry.len, "mtime": format!("{:?}", entry.mtime)}),
                        0,
                    );

                    let mut orch = state.orchestrator.lock().unwrap();
                    let adapter = deadband_core::AdapterCapabilities::default();
                    let (_intervention, _report) = orch.process(event, &adapter);
                    let (loops, interventions, prevented) = if let Some(intervention) = _intervention {
                        tracing::warn!("watchdog loop detected on {:?}: {:?}", path, intervention);
                        (1, 1, 1)
                    } else {
                        (0, 0, 0)
                    };
                    state.record_request(loops, interventions, prevented);
                }
            }
        }

        files = scan;
    }
}

fn scan_dir(dir: &PathBuf) -> Result<HashMap<PathBuf, FileEntry>, std::io::Error> {
    let mut map = HashMap::new();
    scan_recursive(dir, dir, &mut map)?;
    Ok(map)
}

fn scan_recursive(base: &PathBuf, dir: &PathBuf, map: &mut HashMap<PathBuf, FileEntry>) -> Result<(), std::io::Error> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            if !path.file_name().map(|s| s.to_string_lossy().starts_with('.')).unwrap_or(false) {
                scan_recursive(base, &path, map)?;
            }
        } else if path.is_file() {
            let meta = entry.metadata()?;
            let rel = path.strip_prefix(base).unwrap_or(&path).to_path_buf();
            map.insert(rel, FileEntry {
                len: meta.len(),
                mtime: meta.modified().unwrap_or(SystemTime::UNIX_EPOCH),
            });
        }
    }
    Ok(())
}
