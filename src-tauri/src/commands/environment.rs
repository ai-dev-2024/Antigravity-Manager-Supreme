// Environment variable management for Claude Desktop integration
// Handles Windows user environment variables via registry

use tracing::info;
use tauri::Manager;

/// Check if Claude integration is enabled (environment variables are set)
#[tauri::command]
pub async fn get_claude_integration_status(port: u16, api_key: String) -> Result<bool, String> {
    #[cfg(target_os = "windows")]
    {
        use winreg::enums::*;
        use winreg::RegKey;

        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        let env = hkcu.open_subkey("Environment")
            .map_err(|e| format!("Failed to open registry: {}", e))?;

        // Check if both variables exist and match current settings
        let base_url: Result<String, _> = env.get_value("ANTHROPIC_BASE_URL");
        let key: Result<String, _> = env.get_value("ANTHROPIC_API_KEY");

        let expected_url = format!("http://127.0.0.1:{}", port);
        
        match (base_url, key) {
            (Ok(url), Ok(k)) => Ok(url == expected_url && k == api_key),
            _ => Ok(false),
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        // On non-Windows platforms, we can't easily check system env vars
        // Return false and let user manually configure
        Ok(false)
    }
}

/// Enable Claude integration by setting Windows user environment variables
#[tauri::command]
pub async fn enable_claude_integration(port: u16, api_key: String) -> Result<(), String> {
    info!("Enabling Claude integration (port: {}, setting env vars)", port);

    #[cfg(target_os = "windows")]
    {
        use winreg::enums::*;
        use winreg::RegKey;

        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        let (env, _) = hkcu.create_subkey("Environment")
            .map_err(|e| format!("Failed to open registry: {}", e))?;

        let base_url = format!("http://127.0.0.1:{}", port);
        
        env.set_value("ANTHROPIC_BASE_URL", &base_url)
            .map_err(|e| format!("Failed to set ANTHROPIC_BASE_URL: {}", e))?;
        
        env.set_value("ANTHROPIC_API_KEY", &api_key)
            .map_err(|e| format!("Failed to set ANTHROPIC_API_KEY: {}", e))?;

        // Broadcast WM_SETTINGCHANGE so running apps pick up the change
        broadcast_env_change();

        info!("Claude integration enabled: ANTHROPIC_BASE_URL={}, ANTHROPIC_API_KEY=***", base_url);
        Ok(())
    }

    #[cfg(not(target_os = "windows"))]
    {
        Err("Environment variable management is only supported on Windows. Please set ANTHROPIC_BASE_URL and ANTHROPIC_API_KEY manually.".to_string())
    }
}

/// Disable Claude integration by removing Windows user environment variables
#[tauri::command]
pub async fn disable_claude_integration() -> Result<(), String> {
    info!("Disabling Claude integration (removing env vars)");

    #[cfg(target_os = "windows")]
    {
        use winreg::enums::*;
        use winreg::RegKey;

        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        let env = hkcu.open_subkey_with_flags("Environment", KEY_ALL_ACCESS)
            .map_err(|e| format!("Failed to open registry: {}", e))?;

        // Delete the variables (ignore errors if they don't exist)
        let _ = env.delete_value("ANTHROPIC_BASE_URL");
        let _ = env.delete_value("ANTHROPIC_API_KEY");

        // Broadcast WM_SETTINGCHANGE
        broadcast_env_change();

        info!("Claude integration disabled: environment variables removed");
        Ok(())
    }

    #[cfg(not(target_os = "windows"))]
    {
        Err("Environment variable management is only supported on Windows. Please unset ANTHROPIC_BASE_URL and ANTHROPIC_API_KEY manually.".to_string())
    }
}

/// Broadcast WM_SETTINGCHANGE to notify other applications of env var changes
#[cfg(target_os = "windows")]
fn broadcast_env_change() {
    use std::ffi::CString;
    
    // Use Windows API to broadcast the change
    // This requires linking to user32.dll
    #[link(name = "user32")]
    extern "system" {
        fn SendMessageTimeoutA(
            hwnd: *mut std::ffi::c_void,
            msg: u32,
            wparam: usize,
            lparam: *const i8,
            flags: u32,
            timeout: u32,
            result: *mut usize,
        ) -> isize;
    }

    const HWND_BROADCAST: *mut std::ffi::c_void = 0xffff as *mut std::ffi::c_void;
    const WM_SETTINGCHANGE: u32 = 0x001A;
    const SMTO_ABORTIFHUNG: u32 = 0x0002;

    let environment = CString::new("Environment").unwrap();
    let mut result: usize = 0;

    unsafe {
        SendMessageTimeoutA(
            HWND_BROADCAST,
            WM_SETTINGCHANGE,
            0,
            environment.as_ptr(),
            SMTO_ABORTIFHUNG,
            5000,
            &mut result,
        );
    }

    info!("Broadcasted WM_SETTINGCHANGE to notify environment variable update");
}

/// Refresh the Windows environment by restarting explorer.exe
/// This ensures all newly opened applications (including terminals) will have the updated env vars
#[tauri::command]
pub async fn refresh_environment() -> Result<(), String> {
    info!("Refreshing Windows environment by restarting explorer.exe");

    #[cfg(target_os = "windows")]
    {
        use std::process::Command;
        use std::thread;
        use std::time::Duration;

        // First, broadcast the settings change
        broadcast_env_change();

        // Kill explorer.exe gracefully
        let kill_result = Command::new("taskkill")
            .args(["/f", "/im", "explorer.exe"])
            .output();

        if let Err(e) = kill_result {
            return Err(format!("Failed to stop explorer: {}", e));
        }

        // Wait a moment for explorer to fully stop
        thread::sleep(Duration::from_millis(500));

        // Restart explorer.exe
        let start_result = Command::new("explorer.exe")
            .spawn();

        if let Err(e) = start_result {
            // Try to restart explorer anyway even if spawn fails
            let _ = Command::new("cmd")
                .args(["/c", "start", "explorer.exe"])
                .spawn();
            info!("Explorer restart attempted via cmd fallback: {}", e);
        }

        info!("Explorer restarted - environment variables refreshed for all new processes");
        Ok(())
    }

    #[cfg(not(target_os = "windows"))]
    {
        Err("Environment refresh is only supported on Windows.".to_string())
    }
}

/// Configure Claude Desktop to use Antigravity MCP server
/// This modifies the claude_desktop_config.json file to include our MCP server
#[tauri::command]
pub async fn configure_claude_desktop(
    app_handle: tauri::AppHandle,
    port: u16,
    api_key: String,
) -> Result<String, String> {
    #[cfg(any(target_os = "windows", target_os = "macos"))]
    {
        use std::fs;
        use std::path::PathBuf;
        
        info!("Configuring Claude Desktop MCP integration (port: {})", port);
        
        // Get Claude Desktop config path
        #[cfg(target_os = "windows")]
        let config_dir = {
            let appdata = std::env::var("APPDATA")
                .map_err(|_| "Failed to get APPDATA environment variable")?;
            PathBuf::from(appdata).join("Claude")
        };
        
        #[cfg(target_os = "macos")]
        let config_dir = {
            let home = std::env::var("HOME")
                .map_err(|_| "Failed to get HOME environment variable")?;
            PathBuf::from(home).join("Library/Application Support/Claude")
        };
        
        // Ensure config directory exists
        if !config_dir.exists() {
            fs::create_dir_all(&config_dir)
                .map_err(|e| format!("Failed to create Claude config directory: {}", e))?;
        }
        
        let config_path = config_dir.join("claude_desktop_config.json");
        
        // Copy MCP server to user's data directory
        let data_dir = crate::modules::account::get_data_dir()?;
        let mcp_dir = data_dir.join("mcp_server");
        
        if !mcp_dir.exists() {
            fs::create_dir_all(&mcp_dir)
                .map_err(|e| format!("Failed to create MCP server directory: {}", e))?;
        }
        
        // Get resource path for MCP server files
        let resource_path = app_handle
            .path()
            .resource_dir()
            .map_err(|e| format!("Failed to get resource dir: {}", e))?
            .join("resources")
            .join("mcp_server");
        
        // Copy MCP server files
        let mcp_script = mcp_dir.join("antigravity_mcp.py");
        let requirements = mcp_dir.join("requirements.txt");
        
        // If resources exist, copy them; otherwise use embedded content
        if resource_path.join("antigravity_mcp.py").exists() {
            fs::copy(resource_path.join("antigravity_mcp.py"), &mcp_script)
                .map_err(|e| format!("Failed to copy MCP script: {}", e))?;
            fs::copy(resource_path.join("requirements.txt"), &requirements)
                .map_err(|e| format!("Failed to copy requirements: {}", e))?;
        } else {
            // For development: files might be in src-tauri/resources
            let dev_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("resources")
                .join("mcp_server");
            
            if dev_path.join("antigravity_mcp.py").exists() {
                fs::copy(dev_path.join("antigravity_mcp.py"), &mcp_script)
                    .map_err(|e| format!("Failed to copy MCP script (dev): {}", e))?;
                fs::copy(dev_path.join("requirements.txt"), &requirements)
                    .map_err(|e| format!("Failed to copy requirements (dev): {}", e))?;
            } else {
                return Err("MCP server files not found in resources".to_string());
            }
        }
        
        // Auto-install Python MCP dependencies
        info!("Installing Python MCP dependencies...");
        let pip_result = std::process::Command::new("pip")
            .args(["install", "--quiet", "mcp", "httpx"])
            .output();
        
        match pip_result {
            Ok(output) => {
                if output.status.success() {
                    info!("Python MCP dependencies installed successfully");
                } else {
                    // Try pip3 as fallback
                    let pip3_result = std::process::Command::new("pip3")
                        .args(["install", "--quiet", "mcp", "httpx"])
                        .output();
                    
                    match pip3_result {
                        Ok(output3) if output3.status.success() => {
                            info!("Python MCP dependencies installed via pip3");
                        }
                        _ => {
                            info!("Could not auto-install MCP deps, user may need to run: pip install mcp httpx");
                        }
                    }
                }
            }
            Err(_) => {
                info!("pip not found in PATH, user may need to run: pip install mcp httpx");
            }
        }
        
        // Read existing config or create new one
        let mut config: serde_json::Value = if config_path.exists() {
            let content = fs::read_to_string(&config_path)
                .map_err(|e| format!("Failed to read Claude config: {}", e))?;
            serde_json::from_str(&content).unwrap_or(serde_json::json!({}))
        } else {
            serde_json::json!({})
        };
        
        // Ensure mcpServers object exists
        if !config.get("mcpServers").is_some() {
            config["mcpServers"] = serde_json::json!({});
        }
        
        // Add Antigravity MCP server configuration
        let base_url = format!("http://127.0.0.1:{}", port);
        let mcp_script_path = mcp_script.to_string_lossy().replace("\\", "/");
        
        config["mcpServers"]["antigravity-proxy"] = serde_json::json!({
            "command": "python",
            "args": [mcp_script_path],
            "env": {
                "ANTHROPIC_BASE_URL": base_url,
                "ANTHROPIC_API_KEY": api_key
            }
        });
        
        // Write updated config
        let config_str = serde_json::to_string_pretty(&config)
            .map_err(|e| format!("Failed to serialize config: {}", e))?;
        
        fs::write(&config_path, &config_str)
            .map_err(|e| format!("Failed to write Claude config: {}", e))?;
        
        info!("Claude Desktop configured with Antigravity MCP server");
        info!("Config path: {:?}", config_path);
        info!("MCP script path: {:?}", mcp_script);
        
        // Auto-restart Claude Desktop to apply the new MCP configuration
        #[cfg(target_os = "windows")]
        {
            // Kill existing Claude processes
            let _ = std::process::Command::new("taskkill")
                .args(["/F", "/IM", "claude.exe"])
                .output();
            
            // Wait a moment for processes to fully terminate
            std::thread::sleep(std::time::Duration::from_secs(2));
            
            // Try to launch Claude Desktop
            let claude_path = std::path::PathBuf::from(std::env::var("LOCALAPPDATA").unwrap_or_default())
                .join("AnthropicClaude")
                .join("claude.exe");
            
            if claude_path.exists() {
                let _ = std::process::Command::new(&claude_path)
                    .spawn();
                info!("Claude Desktop restarted automatically");
            }
        }
        
        #[cfg(target_os = "macos")]
        {
            // Kill existing Claude processes
            let _ = std::process::Command::new("pkill")
                .args(["-f", "Claude"])
                .output();
            
            std::thread::sleep(std::time::Duration::from_secs(2));
            
            // Try to launch Claude Desktop
            let _ = std::process::Command::new("open")
                .args(["-a", "Claude"])
                .spawn();
            info!("Claude Desktop restarted automatically");
        }
        
        Ok(format!(
            "Claude Desktop configured and restarted successfully!\n\
             Config: {}\n\
             MCP Server: {}\n\n\
             Use the 'ask_claude' tool in Claude Desktop to route requests through your proxy.",
            config_path.display(),
            mcp_script.display()
        ))
    }
    
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        let _ = (app_handle, port, api_key); // Silence unused variable warnings
        Err("Claude Desktop configuration is only supported on Windows and macOS".to_string())
    }
}

/// Check if Claude Desktop MCP is configured
#[tauri::command]
pub async fn get_claude_desktop_status() -> Result<bool, String> {
    #[cfg(any(target_os = "windows", target_os = "macos"))]
    {
        use std::fs;
        use std::path::PathBuf;
        
        #[cfg(target_os = "windows")]
        let config_path = {
            let appdata = std::env::var("APPDATA")
                .map_err(|_| "Failed to get APPDATA environment variable")?;
            PathBuf::from(appdata).join("Claude").join("claude_desktop_config.json")
        };
        
        #[cfg(target_os = "macos")]
        let config_path = {
            let home = std::env::var("HOME")
                .map_err(|_| "Failed to get HOME environment variable")?;
            PathBuf::from(home)
                .join("Library/Application Support/Claude")
                .join("claude_desktop_config.json")
        };
        
        if !config_path.exists() {
            return Ok(false);
        }
        
        let content = fs::read_to_string(&config_path)
            .map_err(|e| format!("Failed to read Claude config: {}", e))?;
        
        let config: serde_json::Value = serde_json::from_str(&content)
            .map_err(|e| format!("Failed to parse Claude config: {}", e))?;
        
        // Check if our MCP server is configured
        Ok(config
            .get("mcpServers")
            .and_then(|servers| servers.get("antigravity-proxy"))
            .is_some())
    }
    
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        Ok(false)
    }
}

// ============================================================
// YOLO Mode - Quick launch Claude CLI with --dangerously-skip-permissions
// ============================================================

/// YOLO Mode Marker - Used to identify Antigravity-managed alias lines
const YOLO_MARKER: &str = "# [Antigravity YOLO Mode]";

/// Enable YOLO mode by adding 'yolo' alias to PowerShell profile
/// This creates a function that sets proxy env vars and runs claude --dangerously-skip-permissions
#[tauri::command]
pub async fn enable_yolo_mode(port: u16, api_key: String) -> Result<String, String> {
    info!("Enabling YOLO mode (port: {}, adding 'yolo' alias to PowerShell profile)", port);

    #[cfg(target_os = "windows")]
    {
        use std::fs;
        use std::path::PathBuf;

        // Get PowerShell profile path
        let profile_path = get_powershell_profile_path()?;
        
        // Ensure profile directory exists
        if let Some(parent) = profile_path.parent() {
            if !parent.exists() {
                fs::create_dir_all(parent)
                    .map_err(|e| format!("Failed to create PowerShell profile directory: {}", e))?;
            }
        }

        // Read existing profile content
        let existing_content = if profile_path.exists() {
            fs::read_to_string(&profile_path)
                .map_err(|e| format!("Failed to read PowerShell profile: {}", e))?
        } else {
            String::new()
        };

        // Remove any existing YOLO mode configuration
        let cleaned_content = remove_yolo_config(&existing_content);

        // Create the new YOLO function with retry settings and Opus models
        let base_url = format!("http://127.0.0.1:{}", port);
        let yolo_function = format!(r#"
{} - Start
# YOLO Mode: Run Claude CLI with combined quota and auto-skip permissions
# Includes automatic retry settings for Google API rate limit handling
# All models set to Opus for maximum reasoning capability
# Usage: Just type 'yolo' in any PowerShell terminal to start Claude in autonomous mode
function yolo {{
    $env:ANTHROPIC_BASE_URL = "{}"
    $env:ANTHROPIC_API_KEY = "{}"
    # Model settings - all set to Opus for best performance
    $env:ANTHROPIC_MODEL = "claude-opus-4-5-thinking"
    $env:CLAUDE_CODE_SUBAGENT_MODEL = "claude-opus-4-5-thinking"
    $env:ANTHROPIC_DEFAULT_HAIKU_MODEL = "claude-opus-4-5-thinking"
    # Retry settings for automatic rate limit recovery
    $env:CLAUDE_CODE_MAX_RETRIES = "50"
    $env:API_TIMEOUT_MS = "120000"
    $env:BASH_MAX_TIMEOUT_MS = "300000"
    Write-Host "🚀 YOLO Mode: Launching Claude with combined Antigravity quota..." -ForegroundColor Yellow
    Write-Host "   Model: claude-opus-4-5-thinking (all agents)" -ForegroundColor Cyan
    Write-Host "   Max Retries: 50 (auto-recovery from rate limits)" -ForegroundColor DarkGray
    claude --dangerously-skip-permissions @args
}}
Set-Alias -Name claude-yolo -Value yolo
{} - End
"#, YOLO_MARKER, base_url, api_key, YOLO_MARKER);

        // Combine cleaned content with new YOLO function
        let new_content = if cleaned_content.trim().is_empty() {
            yolo_function.trim().to_string()
        } else {
            format!("{}\n{}", cleaned_content.trim(), yolo_function)
        };

        // Write updated profile
        fs::write(&profile_path, new_content)
            .map_err(|e| format!("Failed to write PowerShell profile: {}", e))?;

        // Also create yolo.cmd for CMD support (works in all terminals)
        let npm_path = std::env::var("APPDATA")
            .map(|appdata| std::path::PathBuf::from(appdata).join("npm"))
            .map_err(|_| "Failed to get APPDATA environment variable")?;
        
        if npm_path.exists() {
            let yolo_cmd_path = npm_path.join("yolo.cmd");
            let yolo_cmd_content = format!(r#"@echo off
REM YOLO Mode for Claude CLI - Works in CMD, PowerShell, and any terminal
REM Created by Antigravity Manager Supreme

REM Set proxy environment variables
set ANTHROPIC_BASE_URL={}
set ANTHROPIC_API_KEY={}

REM Set all models to Opus for maximum performance
set ANTHROPIC_MODEL=claude-opus-4-5-thinking
set CLAUDE_CODE_SUBAGENT_MODEL=claude-opus-4-5-thinking
set ANTHROPIC_DEFAULT_HAIKU_MODEL=claude-opus-4-5-thinking

REM Retry settings for automatic rate limit recovery
set CLAUDE_CODE_MAX_RETRIES=50
set API_TIMEOUT_MS=120000
set BASH_MAX_TIMEOUT_MS=300000

echo.
echo [33m🚀 YOLO Mode: Launching Claude with combined Antigravity quota...[0m
echo [36m   Model: claude-opus-4-5-thinking (all agents)[0m
echo [90m   Max Retries: 50 (auto-recovery from rate limits)[0m
echo.

REM Launch Claude with all arguments passed through
claude --dangerously-skip-permissions %*
"#, base_url, api_key);
            
            fs::write(&yolo_cmd_path, yolo_cmd_content)
                .map_err(|e| format!("Failed to write yolo.cmd: {}", e))?;
            
            info!("YOLO mode: yolo.cmd created at {:?}", yolo_cmd_path);
        }

        info!("YOLO mode enabled: 'yolo' alias added to {:?}", profile_path);
        
        Ok(format!(
            "YOLO mode enabled! ✅\n\n\
             Works in ALL terminals (CMD, PowerShell, Windows Terminal):\n\
             • 'yolo' - Start Claude in autonomous mode\n\
             • 'yolo \"your task here\"' - Start with a specific task\n\n\
             All models set to Opus for maximum performance.\n\
             Profile updated: {}", 
            profile_path.display()
        ))
    }

    #[cfg(not(target_os = "windows"))]
    {
        let _ = (port, api_key);
        Err("YOLO mode setup is currently only supported on Windows. For macOS/Linux, add the alias to your shell profile manually.".to_string())
    }
}

/// Disable YOLO mode by removing 'yolo' alias from PowerShell profile
#[tauri::command]
pub async fn disable_yolo_mode() -> Result<String, String> {
    info!("Disabling YOLO mode (removing 'yolo' alias from PowerShell profile)");

    #[cfg(target_os = "windows")]
    {
        use std::fs;

        let profile_path = get_powershell_profile_path()?;
        
        if !profile_path.exists() {
            return Ok("YOLO mode was not enabled (no PowerShell profile found).".to_string());
        }

        let existing_content = fs::read_to_string(&profile_path)
            .map_err(|e| format!("Failed to read PowerShell profile: {}", e))?;

        let cleaned_content = remove_yolo_config(&existing_content);

        fs::write(&profile_path, cleaned_content.trim())
            .map_err(|e| format!("Failed to write PowerShell profile: {}", e))?;

        // Also remove yolo.cmd if it exists
        if let Ok(appdata) = std::env::var("APPDATA") {
            let yolo_cmd_path = std::path::PathBuf::from(appdata).join("npm").join("yolo.cmd");
            if yolo_cmd_path.exists() {
                let _ = fs::remove_file(&yolo_cmd_path);
                info!("YOLO mode: yolo.cmd removed from {:?}", yolo_cmd_path);
            }
        }

        info!("YOLO mode disabled: alias removed from {:?}", profile_path);
        
        Ok("YOLO mode disabled! The 'yolo' command has been removed from all terminals.".to_string())
    }

    #[cfg(not(target_os = "windows"))]
    {
        Err("YOLO mode management is currently only supported on Windows.".to_string())
    }
}

/// Check if YOLO mode is enabled
#[tauri::command]
pub async fn get_yolo_mode_status() -> Result<bool, String> {
    #[cfg(target_os = "windows")]
    {
        use std::fs;

        let profile_path = get_powershell_profile_path()?;
        
        if !profile_path.exists() {
            return Ok(false);
        }

        let content = fs::read_to_string(&profile_path)
            .map_err(|e| format!("Failed to read PowerShell profile: {}", e))?;

        Ok(content.contains(YOLO_MARKER))
    }

    #[cfg(not(target_os = "windows"))]
    {
        Ok(false)
    }
}

/// Get PowerShell Core profile path (works with both PowerShell 5.x and 7.x)
#[cfg(target_os = "windows")]
fn get_powershell_profile_path() -> Result<std::path::PathBuf, String> {
    use std::path::PathBuf;

    // Use Documents folder for cross-version compatibility
    let documents = std::env::var("USERPROFILE")
        .map_err(|_| "Failed to get USERPROFILE environment variable")?;
    
    // PowerShell Core (7.x) uses this path, and it also works for Windows PowerShell
    let profile_path = PathBuf::from(documents)
        .join("Documents")
        .join("PowerShell")
        .join("Microsoft.PowerShell_profile.ps1");

    Ok(profile_path)
}

/// Remove existing YOLO mode configuration from profile content
#[cfg(target_os = "windows")]
fn remove_yolo_config(content: &str) -> String {
    let start_marker = format!("{} - Start", YOLO_MARKER);
    let end_marker = format!("{} - End", YOLO_MARKER);
    
    let mut result = String::new();
    let mut in_yolo_block = false;
    
    for line in content.lines() {
        if line.contains(&start_marker) {
            in_yolo_block = true;
            continue;
        }
        if line.contains(&end_marker) {
            in_yolo_block = false;
            continue;
        }
        if !in_yolo_block {
            result.push_str(line);
            result.push('\n');
        }
    }
    
    result
}

// ============================================================
// OpenCode Integration - Configure OpenCode CLI/Desktop to use Antigravity proxy
// ============================================================

/// OpenCode Marker - Used to identify Antigravity-managed sections
const OPENCODE_MARKER: &str = "# [Antigravity OpenCode Integration]";

/// Check if OpenCode integration is enabled
#[tauri::command]
pub async fn get_opencode_integration_status(port: u16, api_key: String) -> Result<bool, String> {
    #[cfg(target_os = "windows")]
    {
        use winreg::enums::*;
        use winreg::RegKey;

        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        let env = hkcu.open_subkey("Environment")
            .map_err(|e| format!("Failed to open registry: {}", e))?;

        // Check if LOCAL_ENDPOINT env var is correctly configured
        let endpoint: Result<String, _> = env.get_value("LOCAL_ENDPOINT");
        let expected_url = format!("http://127.0.0.1:{}/v1", port);
        
        match endpoint {
            Ok(url) => Ok(url == expected_url),
            _ => Ok(false),
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        // On non-Windows, check if opencode.json config exists
        use std::fs;
        use std::path::PathBuf;
        
        let home = std::env::var("HOME").unwrap_or_default();
        let config_path = PathBuf::from(home).join(".config/opencode/opencode.json");
        
        if !config_path.exists() {
            return Ok(false);
        }
        
        let content = fs::read_to_string(&config_path)
            .map_err(|e| format!("Failed to read opencode config: {}", e))?;
        
        // Check if Antigravity provider is configured
        Ok(content.contains("antigravity") && content.contains(&format!("{}", port)))
    }
}

/// Enable OpenCode integration by setting Windows env vars and creating config
#[tauri::command]
pub async fn enable_opencode_integration(port: u16, api_key: String) -> Result<String, String> {
    info!("Enabling OpenCode integration (port: {}, setting env vars and config)", port);

    let base_url = format!("http://127.0.0.1:{}/v1", port);
    
    #[cfg(target_os = "windows")]
    {
        use winreg::enums::*;
        use winreg::RegKey;

        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        let (env, _) = hkcu.create_subkey("Environment")
            .map_err(|e| format!("Failed to open registry: {}", e))?;

        // Set LOCAL_ENDPOINT for OpenCode self-hosted provider support
        env.set_value("LOCAL_ENDPOINT", &base_url)
            .map_err(|e| format!("Failed to set LOCAL_ENDPOINT: {}", e))?;

        // Broadcast WM_SETTINGCHANGE
        broadcast_env_change();

        info!("OpenCode integration enabled: LOCAL_ENDPOINT={}", base_url);
    }

    // Create/update opencode.json config file (works on all platforms)
    create_opencode_config(port, &api_key)?;

    Ok(format!(
        "OpenCode integration enabled! ✅\n\n\
         LOCAL_ENDPOINT set to: {}\n\n\
         Available models via Antigravity:\n\
         • local.gemini-3-flash\n\
         • local.gemini-3-pro\n\
         • local.claude-sonnet-4-5\n\n\
         Run 'opencode' or open OpenCode Desktop to start using your combined quota!",
        base_url
    ))
}

/// Disable OpenCode integration
#[tauri::command]
pub async fn disable_opencode_integration() -> Result<String, String> {
    info!("Disabling OpenCode integration");

    #[cfg(target_os = "windows")]
    {
        use winreg::enums::*;
        use winreg::RegKey;

        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        let env = hkcu.open_subkey_with_flags("Environment", KEY_ALL_ACCESS)
            .map_err(|e| format!("Failed to open registry: {}", e))?;

        // Delete LOCAL_ENDPOINT
        let _ = env.delete_value("LOCAL_ENDPOINT");

        broadcast_env_change();
        info!("OpenCode integration disabled: LOCAL_ENDPOINT removed");
    }

    // Remove Antigravity config from opencode.json
    remove_opencode_config()?;

    Ok("OpenCode integration disabled. LOCAL_ENDPOINT removed and config cleaned.".to_string())
}

/// Create or update opencode.json configuration
fn create_opencode_config(port: u16, api_key: &str) -> Result<(), String> {
    use std::fs;
    use std::path::PathBuf;
    
    // Get config directory
    #[cfg(target_os = "windows")]
    let config_dir = {
        let appdata = std::env::var("USERPROFILE")
            .map_err(|_| "Failed to get USERPROFILE")?;
        PathBuf::from(appdata).join(".config").join("opencode")
    };
    
    #[cfg(not(target_os = "windows"))]
    let config_dir = {
        let home = std::env::var("HOME")
            .map_err(|_| "Failed to get HOME")?;
        PathBuf::from(home).join(".config").join("opencode")
    };
    
    // Ensure config directory exists
    if !config_dir.exists() {
        fs::create_dir_all(&config_dir)
            .map_err(|e| format!("Failed to create opencode config directory: {}", e))?;
    }
    
    let config_path = config_dir.join("opencode.json");
    let base_url = format!("http://127.0.0.1:{}/v1", port);
    
    // Read existing config or create new
    let mut config: serde_json::Value = if config_path.exists() {
        let content = fs::read_to_string(&config_path)
            .map_err(|e| format!("Failed to read opencode config: {}", e))?;
        serde_json::from_str(&content).unwrap_or(serde_json::json!({}))
    } else {
        serde_json::json!({})
    };
    
    // Add Antigravity as a custom provider
    // OpenCode uses "providers" for custom endpoints
    if !config.get("providers").is_some() {
        config["providers"] = serde_json::json!({});
    }
    
    config["providers"]["antigravity"] = serde_json::json!({
        "name": "Antigravity Manager",
        "type": "openai",
        "baseURL": base_url,
        "apiKey": api_key,
        "models": [
            "gemini-3-flash",
            "gemini-3-pro", 
            "gemini-3-pro-high",
            "claude-sonnet-4-5",
            "claude-sonnet-4-5-thinking",
            "claude-opus-4-5-thinking"
        ]
    });
    
    // Set default agent to use Antigravity
    if !config.get("agents").is_some() {
        config["agents"] = serde_json::json!({});
    }
    
    config["agents"]["coder"] = serde_json::json!({
        "model": "local.gemini-3-flash",
        "reasoningEffort": "high"
    });
    
    // Write config
    let config_str = serde_json::to_string_pretty(&config)
        .map_err(|e| format!("Failed to serialize config: {}", e))?;
    
    fs::write(&config_path, config_str)
        .map_err(|e| format!("Failed to write opencode config: {}", e))?;
    
    info!("OpenCode config created at {:?}", config_path);
    Ok(())
}

/// Remove Antigravity config from opencode.json
fn remove_opencode_config() -> Result<(), String> {
    use std::fs;
    use std::path::PathBuf;
    
    #[cfg(target_os = "windows")]
    let config_path = {
        let appdata = std::env::var("USERPROFILE").unwrap_or_default();
        PathBuf::from(appdata).join(".config").join("opencode").join("opencode.json")
    };
    
    #[cfg(not(target_os = "windows"))]
    let config_path = {
        let home = std::env::var("HOME").unwrap_or_default();
        PathBuf::from(home).join(".config").join("opencode").join("opencode.json")
    };
    
    if !config_path.exists() {
        return Ok(());
    }
    
    let content = fs::read_to_string(&config_path)
        .map_err(|e| format!("Failed to read opencode config: {}", e))?;
    
    let mut config: serde_json::Value = serde_json::from_str(&content)
        .map_err(|e| format!("Failed to parse opencode config: {}", e))?;
    
    // Remove Antigravity provider
    if let Some(providers) = config.get_mut("providers") {
        if let Some(obj) = providers.as_object_mut() {
            obj.remove("antigravity");
        }
    }
    
    // Remove Antigravity agent config if it was using local models
    if let Some(agents) = config.get_mut("agents") {
        if let Some(coder) = agents.get("coder") {
            if let Some(model) = coder.get("model") {
                if model.as_str().unwrap_or("").starts_with("local.") {
                    if let Some(obj) = agents.as_object_mut() {
                        obj.remove("coder");
                    }
                }
            }
        }
    }
    
    let config_str = serde_json::to_string_pretty(&config)
        .map_err(|e| format!("Failed to serialize config: {}", e))?;
    
    fs::write(&config_path, config_str)
        .map_err(|e| format!("Failed to write opencode config: {}", e))?;
    
    info!("Antigravity config removed from opencode.json");
    Ok(())
}

/// Enable YOLO mode for OpenCode (PowerShell alias)
#[tauri::command]
pub async fn enable_opencode_yolo_mode(port: u16, api_key: String) -> Result<String, String> {
    info!("Enabling OpenCode YOLO mode (port: {})", port);

    #[cfg(target_os = "windows")]
    {
        use std::fs;

        let profile_path = get_powershell_profile_path()?;
        
        // Ensure profile directory exists
        if let Some(parent) = profile_path.parent() {
            if !parent.exists() {
                fs::create_dir_all(parent)
                    .map_err(|e| format!("Failed to create profile directory: {}", e))?;
            }
        }

        let existing_content = if profile_path.exists() {
            fs::read_to_string(&profile_path)
                .map_err(|e| format!("Failed to read profile: {}", e))?
        } else {
            String::new()
        };

        // Remove existing OpenCode YOLO config
        let cleaned_content = remove_opencode_yolo_config(&existing_content);

        let base_url = format!("http://127.0.0.1:{}/v1", port);
        let opencode_function = format!(r#"
{} - Start
# OpenCode YOLO Mode: Run OpenCode with Antigravity combined quota
# Usage: Just type 'opencode-yolo' in any PowerShell terminal
function opencode-yolo {{
    $env:LOCAL_ENDPOINT = "{}"
    $env:OPENAI_API_KEY = "{}"
    Write-Host "🚀 OpenCode YOLO: Launching with Antigravity combined quota..." -ForegroundColor Yellow
    Write-Host "   Endpoint: $env:LOCAL_ENDPOINT" -ForegroundColor Cyan
    opencode @args
}}
Set-Alias -Name oc-yolo -Value opencode-yolo
{} - End
"#, OPENCODE_MARKER, base_url, api_key, OPENCODE_MARKER);

        let new_content = if cleaned_content.trim().is_empty() {
            opencode_function.trim().to_string()
        } else {
            format!("{}\n{}", cleaned_content.trim(), opencode_function)
        };

        fs::write(&profile_path, new_content)
            .map_err(|e| format!("Failed to write profile: {}", e))?;

        info!("OpenCode YOLO mode enabled in {:?}", profile_path);
        
        Ok(format!(
            "OpenCode YOLO mode enabled! ✅\n\n\
             Commands available:\n\
             • 'opencode-yolo' - Start OpenCode with Antigravity\n\
             • 'oc-yolo' - Short alias\n\n\
             Profile updated: {}",
            profile_path.display()
        ))
    }

    #[cfg(not(target_os = "windows"))]
    {
        let _ = (port, api_key);
        Err("OpenCode YOLO mode is only supported on Windows. Set LOCAL_ENDPOINT manually on other platforms.".to_string())
    }
}

/// Remove OpenCode YOLO config from profile
#[cfg(target_os = "windows")]
fn remove_opencode_yolo_config(content: &str) -> String {
    let start_marker = format!("{} - Start", OPENCODE_MARKER);
    let end_marker = format!("{} - End", OPENCODE_MARKER);
    
    let mut result = String::new();
    let mut in_block = false;
    
    for line in content.lines() {
        if line.contains(&start_marker) {
            in_block = true;
            continue;
        }
        if line.contains(&end_marker) {
            in_block = false;
            continue;
        }
        if !in_block {
            result.push_str(line);
            result.push('\n');
        }
    }
    
    result
}

/// Disable OpenCode YOLO mode
#[tauri::command]
pub async fn disable_opencode_yolo_mode() -> Result<String, String> {
    info!("Disabling OpenCode YOLO mode");

    #[cfg(target_os = "windows")]
    {
        use std::fs;

        let profile_path = get_powershell_profile_path()?;
        
        if !profile_path.exists() {
            return Ok("OpenCode YOLO mode was not enabled.".to_string());
        }

        let existing_content = fs::read_to_string(&profile_path)
            .map_err(|e| format!("Failed to read profile: {}", e))?;

        let cleaned_content = remove_opencode_yolo_config(&existing_content);

        fs::write(&profile_path, cleaned_content.trim())
            .map_err(|e| format!("Failed to write profile: {}", e))?;

        info!("OpenCode YOLO mode disabled");
        Ok("OpenCode YOLO mode disabled. Aliases removed.".to_string())
    }

    #[cfg(not(target_os = "windows"))]
    {
        Err("OpenCode YOLO mode management only supported on Windows.".to_string())
    }
}

/// Get OpenCode YOLO mode status
#[tauri::command]
pub async fn get_opencode_yolo_status() -> Result<bool, String> {
    #[cfg(target_os = "windows")]
    {
        use std::fs;

        let profile_path = get_powershell_profile_path()?;
        
        if !profile_path.exists() {
            return Ok(false);
        }

        let content = fs::read_to_string(&profile_path)
            .map_err(|e| format!("Failed to read profile: {}", e))?;

        Ok(content.contains(OPENCODE_MARKER))
    }

    #[cfg(not(target_os = "windows"))]
    {
        Ok(false)
    }
}

