use std::path::PathBuf;

use anyhow::{Context, Result};

pub struct ServiceManager;

impl ServiceManager {
    /// Install the proxy as a system service (launchd on macOS, systemd on Linux).
    pub fn install(port: u16, _binary_path: &PathBuf) -> Result<()> {
        #[cfg(target_os = "macos")]
        {
            Self::install_launchd(port, _binary_path)?;
        }
        #[cfg(target_os = "linux")]
        {
            Self::install_systemd(port, _binary_path)?;
        }
        #[cfg(target_os = "windows")]
        {
            Self::install_windows_service(port, _binary_path)?;
        }
        Ok(())
    }

    /// Remove the system service.
    pub fn uninstall() -> Result<()> {
        #[cfg(target_os = "macos")]
        {
            Self::uninstall_launchd()?;
        }
        #[cfg(target_os = "linux")]
        {
            Self::uninstall_systemd()?;
        }
        #[cfg(target_os = "windows")]
        {
            Self::uninstall_windows_service()?;
        }
        Ok(())
    }

    /// Check if the service is installed and running.
    pub fn status() -> Result<ServiceStatus> {
        #[cfg(target_os = "macos")]
        {
            Self::check_launchd()
        }
        #[cfg(target_os = "linux")]
        {
            Self::check_systemd()
        }
        #[cfg(target_os = "windows")]
        {
            Self::check_windows_service()
        }
        #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
        {
            Ok(ServiceStatus::NotSupported)
        }
    }

    // --- macOS launchd ---
    #[cfg(target_os = "macos")]
    fn install_launchd(port: u16, binary_path: &PathBuf) -> Result<()> {
        let plist_path = Self::launchd_plist_path();
        let data_dir = crate::config::ProxyConfig::data_dir();

        let plist = format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>com.deadband.proxy</string>
    <key>ProgramArguments</key>
    <array>
        <string>{}</string>
        <string>proxy</string>
        <string>--port</string>
        <string>{}</string>
        <string>--daemon</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <true/>
    <key>StandardOutPath</key>
    <string>{}/logs/deadband.log</string>
    <key>StandardErrorPath</key>
    <string>{}/logs/deadband.err</string>
    <key>WorkingDirectory</key>
    <string>{}</string>
</dict>
</plist>"#,
            binary_path.display(),
            port,
            data_dir.display(),
            data_dir.display(),
            data_dir.display(),
        );

        std::fs::create_dir_all(plist_path.parent().unwrap())
            .with_context(|| format!("Failed to create launchd dir: {:?}", plist_path.parent()))?;
        std::fs::write(&plist_path, plist)
            .with_context(|| format!("Failed to write launchd plist: {:?}", plist_path))?;

        // Load the service
        std::process::Command::new("launchctl")
            .args(["load", &plist_path.to_string_lossy()])
            .output()
            .context("Failed to load launchd service")?;

        tracing::info!("Installed Deadband Proxy as launchd service");
        Ok(())
    }

    #[cfg(target_os = "macos")]
    fn uninstall_launchd() -> Result<()> {
        let plist_path = Self::launchd_plist_path();
        if plist_path.exists() {
            std::process::Command::new("launchctl")
                .args(["unload", &plist_path.to_string_lossy()])
                .output()
                .context("Failed to unload launchd service")?;
            std::fs::remove_file(&plist_path)
                .with_context(|| format!("Failed to remove plist: {:?}", plist_path))?;
            tracing::info!("Uninstalled Deadband Proxy launchd service");
        }
        Ok(())
    }

    #[cfg(target_os = "macos")]
    fn check_launchd() -> Result<ServiceStatus> {
        let plist_path = Self::launchd_plist_path();
        if !plist_path.exists() {
            return Ok(ServiceStatus::NotInstalled);
        }

        let output = std::process::Command::new("launchctl")
            .args(["list", "com.deadband.proxy"])
            .output()
            .context("Failed to check launchd service")?;

        if output.status.success() {
            Ok(ServiceStatus::Running)
        } else {
            Ok(ServiceStatus::Stopped)
        }
    }

    #[cfg(target_os = "macos")]
    fn launchd_plist_path() -> PathBuf {
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
        home.join("Library")
            .join("LaunchAgents")
            .join("com.deadband.proxy.plist")
    }

    // --- Linux systemd ---
    #[cfg(target_os = "linux")]
    fn install_systemd(port: u16, binary_path: &PathBuf) -> Result<()> {
        let service_path = PathBuf::from("/etc/systemd/system/deadband-proxy.service");
        let data_dir = crate::config::ProxyConfig::data_dir();

        let service = format!(
            r#"[Unit]
Description=Deadband Proxy - AI Agent Loop Protection
After=network.target

[Service]
Type=simple
ExecStart={} proxy --port {} --daemon
Restart=on-failure
RestartSec=5
StandardOutput=append:{}/logs/deadband.log
StandardError=append:{}/logs/deadband.err
WorkingDirectory={}

[Install]
WantedBy=multi-user.target
"#,
            binary_path.display(),
            port,
            data_dir.display(),
            data_dir.display(),
            data_dir.display(),
        );

        std::fs::write(&service_path, service)
            .with_context(|| format!("Failed to write systemd service: {:?}", service_path))?;

        std::process::Command::new("systemctl")
            .args(["daemon-reload"])
            .output()
            .context("Failed to reload systemd")?;
        std::process::Command::new("systemctl")
            .args(["enable", "deadband-proxy"])
            .output()
            .context("Failed to enable systemd service")?;
        std::process::Command::new("systemctl")
            .args(["start", "deadband-proxy"])
            .output()
            .context("Failed to start systemd service")?;

        tracing::info!("Installed Deadband Proxy as systemd service");
        Ok(())
    }

    #[cfg(target_os = "linux")]
    fn uninstall_systemd() -> Result<()> {
        let service_path = PathBuf::from("/etc/systemd/system/deadband-proxy.service");
        if service_path.exists() {
            std::process::Command::new("systemctl")
                .args(["stop", "deadband-proxy"])
                .output().ok();
            std::process::Command::new("systemctl")
                .args(["disable", "deadband-proxy"])
                .output().ok();
            std::fs::remove_file(&service_path).ok();
            std::process::Command::new("systemctl")
                .args(["daemon-reload"])
                .output().ok();
            tracing::info!("Uninstalled Deadband Proxy systemd service");
        }
        Ok(())
    }

    #[cfg(target_os = "linux")]
    fn check_systemd() -> Result<ServiceStatus> {
        let output = std::process::Command::new("systemctl")
            .args(["is-active", "deadband-proxy"])
            .output()
            .context("Failed to check systemd service")?;
        let status = String::from_utf8_lossy(&output.stdout).trim().to_string();
        match status.as_str() {
            "active" => Ok(ServiceStatus::Running),
            "inactive" | "dead" => Ok(ServiceStatus::Stopped),
            _ => Ok(ServiceStatus::NotInstalled),
        }
    }

    // --- Windows Service ---
    #[cfg(target_os = "windows")]
    fn install_windows_service(_port: u16, _binary_path: &PathBuf) -> Result<()> {
        tracing::info!("Windows service installation not yet implemented");
        Ok(())
    }

    #[cfg(target_os = "windows")]
    fn uninstall_windows_service() -> Result<()> {
        tracing::info!("Windows service uninstallation not yet implemented");
        Ok(())
    }

    #[cfg(target_os = "windows")]
    fn check_windows_service() -> Result<ServiceStatus> {
        Ok(ServiceStatus::NotSupported)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum ServiceStatus {
    Running,
    Stopped,
    NotInstalled,
    NotSupported,
}
