use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncRead, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::RwLock;
use tracing::{debug, info};

#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;

#[cfg(target_os = "windows")]
const CREATE_NO_WINDOW: u32 = 0x08000000;

/// CloudflaredtunnelMode
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum TunnelMode {
    /// fast tunnel(TemporaryURL)
    Quick,
    /// Authenticatetunnel(UsingToken)
    Auth,
}

impl Default for TunnelMode {
    fn default() -> Self {
        Self::Quick
    }
}

/// CloudflaredConfig
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudflaredConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub mode: TunnelMode,
    /// Proxyof localPort
    pub port: u16,
    /// AuthenticateMode的Token
    #[serde(default)]
    pub token: Option<String>,
    /// Usinghttp2Protocol(更Compatible)
    #[serde(default)]
    pub use_http2: bool,
}

impl Default for CloudflaredConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            mode: TunnelMode::Quick,
            port: 8045,
            token: None,
            use_http2: true, // DefaultEnablehttp2，更Stable
        }
    }
}

/// CloudflaredStatus
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudflaredStatus {
    pub installed: bool,
    pub version: Option<String>,
    pub running: bool,
    pub url: Option<String>,
    pub error: Option<String>,
}

impl Default for CloudflaredStatus {
    fn default() -> Self {
        Self {
            installed: false,
            version: None,
            running: false,
            url: None,
            error: None,
        }
    }
}

/// CloudflaredManagerStatus
pub struct CloudflaredManager {
    process: Arc<RwLock<Option<Child>>>,
    status: Arc<RwLock<CloudflaredStatus>>,
    bin_path: PathBuf,
    /// used forNotificationProcessmonitorTaskStop
    shutdown_tx: RwLock<Option<tokio::sync::oneshot::Sender<()>>>,
}

impl CloudflaredManager {
    pub fn new(data_dir: &PathBuf) -> Self {
        let bin_name = if cfg!(target_os = "windows") {
            "cloudflared.exe"
        } else {
            "cloudflared"
        };
        let bin_path = data_dir.join("bin").join(bin_name);

        Self {
            process: Arc::new(RwLock::new(None)),
            status: Arc::new(RwLock::new(CloudflaredStatus::default())),
            bin_path,
            shutdown_tx: RwLock::new(None),
        }
    }

    /// CheckYesNoInstalled
    pub async fn check_installed(&self) -> (bool, Option<String>) {
        if !self.bin_path.exists() {
            return (false, None);
        }

        let mut cmd = Command::new(&self.bin_path);
        cmd.arg("--version");
        #[cfg(target_os = "windows")]
        cmd.creation_flags(CREATE_NO_WINDOW);
        
        match cmd.output().await {
            Ok(output) => {
                if output.status.success() {
                    let version = String::from_utf8_lossy(&output.stdout)
                        .lines()
                        .next()
                        .map(|s| s.trim().to_string());
                    (true, version)
                } else {
                    (false, None)
                }
            }
            Err(_) => (false, None),
        }
    }

    /// GetCurrentStatus
    pub async fn get_status(&self) -> CloudflaredStatus {
        self.status.read().await.clone()
    }

    /// UpdateStatus
    async fn update_status(&self, f: impl FnOnce(&mut CloudflaredStatus)) {
        let mut status = self.status.write().await;
        f(&mut status);
    }

    /// Installcloudflared
    pub async fn install(&self) -> Result<CloudflaredStatus, String> {
        let bin_dir = self.bin_path.parent().unwrap();
        if !bin_dir.exists() {
            std::fs::create_dir_all(bin_dir)
                .map_err(|e| format!("Failed to create bin directory: {}", e))?;
        }

        let download_url = get_download_url()?;
        info!("[cloudflared] Downloading from: {}", download_url);

        let response = reqwest::get(&download_url)
            .await
            .map_err(|e| format!("Download failed: {}", e))?;

        if !response.status().is_success() {
            return Err(format!("Download failed with status: {}", response.status()));
        }

        let bytes = response
            .bytes()
            .await
            .map_err(|e| format!("Failed to read response: {}", e))?;

        let is_archive = download_url.ends_with(".tgz");
        if is_archive {
            let archive_path = self.bin_path.with_extension("tgz");
            std::fs::write(&archive_path, &bytes)
                .map_err(|e| format!("Failed to write archive: {}", e))?;

            let status = Command::new("tar")
                .arg("-xzf")
                .arg(&archive_path)
                .arg("-C")
                .arg(bin_dir)
                .status()
                .await
                .map_err(|e| format!("Failed to extract archive: {}", e))?;

            if !status.success() {
                return Err("Failed to extract cloudflared archive".to_string());
            }

            let _ = std::fs::remove_file(&archive_path);
        } else {
            std::fs::write(&self.bin_path, &bytes)
                .map_err(|e| format!("Failed to write binary: {}", e))?;
        }

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&self.bin_path, std::fs::Permissions::from_mode(0o755))
                .map_err(|e| format!("Failed to set permissions: {}", e))?;
        }

        let (installed, version) = self.check_installed().await;
        self.update_status(|s| {
            s.installed = installed;
            s.version = version.clone();
        }).await;

        info!("[cloudflared] Installed successfully, version: {:?}", version);
        Ok(self.get_status().await)
    }

    /// Starttunnel
    pub async fn start(&self, config: CloudflaredConfig) -> Result<CloudflaredStatus, String> {
        // CheckYesNoAlready inRun
        {
            let proc = self.process.read().await;
            if proc.is_some() {
                return Ok(self.get_status().await);
            }
        }

        // StopBeforemonitoringTask
        if let Some(tx) = self.shutdown_tx.write().await.take() {
            let _ = tx.send(());
        }

        let (installed, version) = self.check_installed().await;
        if !installed {
            return Err("Cloudflared not installed".to_string());
        }

        let local_url = format!("http://localhost:{}", config.port);
        info!("[cloudflared] Starting tunnel to: {}", local_url);

        let mut cmd = Command::new(&self.bin_path);

        match config.mode {
            TunnelMode::Quick => {
                cmd.arg("tunnel")
                    .arg("--url")
                    .arg(&local_url)
                    .arg("--no-autoupdate"); // Disable automaticUpdateAvoid unexpected reboots
                if config.use_http2 {
                    cmd.arg("--protocol").arg("http2");
                }
                cmd.arg("--loglevel").arg("info"); // OutputMoreLogEasy to troubleshoot
            }
            TunnelMode::Auth => {
                if let Some(token) = &config.token {
                    cmd.arg("tunnel")
                        .arg("run")
                        .arg("--token")
                        .arg(token)
                        .arg("--no-autoupdate");
                    if config.use_http2 {
                        cmd.arg("--protocol").arg("http2");
                    }
                    cmd.arg("--loglevel").arg("info");
                } else {
                    return Err("Token required for auth mode".to_string());
                }
            }
        }

        cmd.stdout(Stdio::piped()).stderr(Stdio::piped());

        let mut child = cmd.spawn().map_err(|e| format!("Failed to spawn: {}", e))?;

        let stdout = child.stdout.take();
        let stderr = child.stderr.take();

        let status_clone = self.status.clone();
        if let Some(stdout) = stdout {
            spawn_log_reader(stdout, status_clone.clone());
        }

        if let Some(stderr) = stderr {
            spawn_log_reader(stderr, status_clone.clone());
        }

        *self.process.write().await = Some(child);
        self.update_status(|s| {
            s.installed = installed.clone();
            s.version = version.clone();
            s.running = true;
            s.error = None;
        }).await;

        // StartProcessmonitorTask
        let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel::<()>();
        *self.shutdown_tx.write().await = Some(shutdown_tx);

        let process_ref = self.process.clone();
        let status_ref = self.status.clone();

        tokio::spawn(async move {
            tokio::select! {
                _ = shutdown_rx => {
                    debug!("[cloudflared] Process monitor shutdown");
                }
                _ = async {
                    loop {
                        tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;

                        let mut proc_lock = process_ref.write().await;
                        if let Some(ref mut child) = *proc_lock {
                            match child.try_wait() {
                                Ok(Some(exit_status)) => {
                                    // ProcessExited
                                    info!("[cloudflared] Process exited with status: {:?}", exit_status);
                                    *proc_lock = None;
                                    drop(proc_lock);

                                    let mut s = status_ref.write().await;
                                    s.running = false;
                                    s.error = Some(format!("Tunnel process exited (status: {:?})", exit_status));
                                    break;
                                }
                                Ok(None) => {
                                    // ProcessstillRun
                                }
                                Err(e) => {
                                    info!("[cloudflared] Error checking process: {}", e);
                                    *proc_lock = None;
                                    drop(proc_lock);

                                    let mut s = status_ref.write().await;
                                    s.running = false;
                                    s.error = Some(format!("Error checking tunnel: {}", e));
                                    break;
                                }
                            }
                        } else {
                            // ProcessDoes not exist
                            drop(proc_lock);
                            let mut s = status_ref.write().await;
                            if s.running {
                                s.running = false;
                                s.error = Some("Tunnel process not found".to_string());
                            }
                            break;
                        }
                    }
                } => {}
            }
        });

        Ok(self.get_status().await)
    }

    /// Stoptunnel
    pub async fn stop(&self) -> Result<CloudflaredStatus, String> {
        let mut proc_lock = self.process.write().await;
        if let Some(mut child) = proc_lock.take() {
            let _ = child.kill().await;
            info!("[cloudflared] Tunnel stopped");
        }

        self.update_status(|s| {
            s.running = false;
            s.url = None;
            s.error = None;
        }).await;

        Ok(self.get_status().await)
    }
}

/// GetDownloadURL
fn get_download_url() -> Result<String, String> {
    let os = std::env::consts::OS;
    let arch = std::env::consts::ARCH;

    let (os_str, arch_str, ext) = match (os, arch) {
        ("macos", "aarch64") => ("darwin", "arm64", ".tgz"),
        ("macos", "x86_64") => ("darwin", "amd64", ".tgz"),
        ("linux", "x86_64") => ("linux", "amd64", ""),
        ("linux", "aarch64") => ("linux", "arm64", ""),
        ("windows", "x86_64") => ("windows", "amd64", ".exe"),
        _ => return Err(format!("Unsupported platform: {}-{}", os, arch)),
    };

    Ok(format!(
        "https://github.com/cloudflare/cloudflared/releases/latest/download/cloudflared-{}-{}{}",
        os_str, arch_str, ext
    ))
}

fn spawn_log_reader<R>(stream: R, status_ref: Arc<RwLock<CloudflaredStatus>>)
where
    R: AsyncRead + Unpin + Send + 'static,
{
    tokio::spawn(async move {
        let reader = BufReader::new(stream);
        let mut lines = reader.lines();
        while let Ok(Some(line)) = lines.next_line().await {
            debug!("[cloudflared] {}", line);
            if let Some(url) = extract_tunnel_url(&line) {
                info!("[cloudflared] Tunnel URL: {}", url);
                let mut s = status_ref.write().await;
                s.url = Some(url);
            }
        }
    });
}

/// 从LogLineExtract tunnelURL
fn extract_tunnel_url(line: &str) -> Option<String> {
    line.split_whitespace()
        .find(|s| s.starts_with("https://") && s.contains(".trycloudflare.com"))
        .map(|s| s.to_string())
}

