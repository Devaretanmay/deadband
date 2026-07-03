// Utility functions for Deadband wrap command

use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};
use anyhow::{Context, Result};

/// Check if proxy is running on given port
pub fn check_proxy_running(port: u16) -> bool {
    TcpStream::connect(format!("127.0.0.1:{}", port)).is_ok()
}

/// Start deadband proxy as background process
pub fn start_proxy(port: u16) -> Result<Child> {
    let mut child = Command::new("deadband")
        .arg("enable")
        .arg("--port")
        .arg(port.to_string())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .context(format!("Failed to start deadband proxy on port {}", port))?;
    
    // Wait for proxy to be ready (timeout: 45s)
    let timeout = Duration::from_secs(45);
    let start = Instant::now();
    
    while start.elapsed() < timeout {
        if check_proxy_running(port) {
            return Ok(child);
        }
        std::thread::sleep(Duration::from_millis(1000));
    }
    
    child.kill()?;
    anyhow::bail!(
        "Proxy failed to start on port {} within {} seconds. \
         Is another service using this port?",
        port, 
        timeout.as_secs()
    );
}

/// Get current directory name as project identifier
pub fn get_project_name() -> Option<String> {
    std::env::current_dir()
        .ok()
        .map(|p| {
            p.file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default()
        })
}

/// Get Deadband data directory
pub fn get_data_dir() -> Result<PathBuf> {
    let home = dirs::home_dir()
        .ok_or_else(|| anyhow::anyhow!("Cannot find home directory"))?;
    Ok(home.join(".deadband"))
}

/// Create backup of a config file
/// If backup already exists, it won't be overwritten
pub fn backup_file(path: &Path) -> Result<PathBuf> {
    if !path.exists() {
        return Ok(PathBuf::from(path));
    }
    
    let extension = path.extension()
        .map(|ext| format!("{}.deadband-backup", ext.to_string_lossy()))
        .unwrap_or_else(|| "deadband-backup".to_string());
    
    let backup_path = path.with_extension(extension);
    
    if !backup_path.exists() {
        std::fs::copy(path, &backup_path)
            .with_context(|| format!("Failed to create backup of {}", path.display()))?;
    }
    
    Ok(backup_path)
}

/// Restore config file from backup
/// Removes the backup file after successful restore
pub fn restore_file(backup_path: &Path) -> Result<()> {
    if backup_path.exists() {
        let original_path = backup_path.with_extension("");
        std::fs::copy(backup_path, &original_path)
            .with_context(|| format!("Failed to restore {}", original_path.display()))?;
        std::fs::remove_file(backup_path)
            .with_context(|| format!("Failed to remove backup {}", backup_path.display()))?;
    }
    Ok(())
}

/// Get config directory path for a specific app
pub fn get_config_dir(app_name: &str) -> Result<PathBuf> {
    let config_dir = dirs::config_dir()
        .ok_or_else(|| anyhow::anyhow!("Cannot find config directory"))?;
    Ok(config_dir.join(app_name))
}

/// Get home directory
pub fn get_home_dir() -> Result<PathBuf> {
    dirs::home_dir()
        .ok_or_else(|| anyhow::anyhow!("Cannot find home directory"))
}

/// Ensure a directory exists
pub fn ensure_dir(path: &Path) -> Result<()> {
    std::fs::create_dir_all(path)
        .with_context(|| format!("Failed to create directory {}", path.display()))
}

/// Write JSON to a file
pub fn write_json<T: serde::Serialize>(path: &Path, data: &T) -> Result<()> {
    let json = serde_json::to_string_pretty(data)
        .context("Failed to serialize to JSON")?;
    std::fs::write(path, json)
        .with_context(|| format!("Failed to write JSON to {}", path.display()))
}

/// Read JSON from a file
pub fn read_json<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T> {
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read file {}", path.display()))?;
    serde_json::from_str(&content)
        .with_context(|| format!("Failed to parse JSON from {}", path.display()))
}

/// Parse JSON loosely (handles comments in JSON)
/// Simple implementation that removes single-line comments only
pub fn parse_json_loose(content: &str) -> Result<serde_json::Value> {
    // Try standard JSON first
    match serde_json::from_str(content) {
        Ok(value) => Ok(value),
        Err(_) => {
            // Try stripping single-line comments
            let cleaned = strip_single_line_comments(content);
            serde_json::from_str(&cleaned)
                .context("Failed to parse JSON even after stripping comments")
        }
    }
}

/// Strip single-line comments (// ...) from JSON
fn strip_single_line_comments(content: &str) -> String {
    content.lines()
        .map(|line| {
            if let Some(pos) = line.find("//") {
                &line[..pos]
            } else {
                line
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use tempfile::NamedTempFile;
    
    #[test]
    fn test_backup_and_restore() {
        // Create a temp file
        let mut file = NamedTempFile::new().unwrap();
        let path = file.path().to_path_buf();
        
        // Write some content
        writeln!(file, "test content").unwrap();
        file.flush().unwrap();
        
        // Backup
        let backup_path = backup_file(&path).unwrap();
        assert!(backup_path.exists());
        
        // Modify original
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .open(&path)
            .unwrap();
        writeln!(file, "modified content").unwrap();
        file.flush().unwrap();
        
        // Restore
        restore_file(&backup_path).unwrap();
        
        // Check content restored
        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("test content"));
        assert!(!content.contains("modified content"));
        
        // Backup should be removed
        assert!(!backup_path.exists());
    }
    
    #[test]
    fn test_get_project_name() {
        let original_dir = std::env::current_dir().unwrap();
        
        // Change to a temp directory
        let temp_dir = tempfile::tempdir().unwrap();
        std::env::set_current_dir(temp_dir.path()).unwrap();
        
        let name = get_project_name().unwrap();
        assert_eq!(name, temp_dir.path().file_name().unwrap().to_string_lossy());
        
        // Restore original directory
        std::env::set_current_dir(original_dir).unwrap();
    }
}
