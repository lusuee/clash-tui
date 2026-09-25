use std::path::{Path, PathBuf};
use std::time::Duration;

pub struct CoreManager {
    pub bin_path: PathBuf,
    pub data_dir: PathBuf,
}

impl CoreManager {
    pub fn new() -> Self {
        #[cfg(target_os = "windows")]
        let bin_name = "mihomo.exe";
        #[cfg(not(target_os = "windows"))]
        let bin_name = "mihomo";

        let primary_path = Path::new("bin").join(bin_name);
        let nested_path = Path::new("bin").join("mihomo").join(bin_name);
        let bin_path = if !primary_path.is_file() && nested_path.is_file() {
            nested_path
        } else {
            primary_path
        };
        let data_dir = PathBuf::from("data");

        let _ = std::fs::create_dir_all("bin");
        let _ = std::fs::create_dir_all(&data_dir);

        Self { bin_path, data_dir }
    }

    pub fn is_installed(&self) -> bool {
        self.bin_path.is_file()
    }

    pub async fn is_running(&self, api_url: &str, secret: Option<&str>) -> bool {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_millis(600))
            .build();

        let client = match client {
            Ok(c) => c,
            Err(_) => return false,
        };

        let url = format!("{}/version", api_url.trim_end_matches('/'));
        let mut req = client.get(&url);
        if let Some(s) = secret {
            if !s.is_empty() {
                req = req.header("Authorization", format!("Bearer {}", s));
            }
        }

        match req.send().await {
            Ok(resp) => resp.status().is_success(),
            Err(_) => false,
        }
    }

    pub fn ensure_default_config(&self) {
        let config_path = self.data_dir.join("config.yaml");
        if !config_path.exists() {
            let default_yaml = "mixed-port: 7897\n\
                                allow-lan: false\n\
                                mode: rule\n\
                                log-level: info\n\
                                external-controller: 127.0.0.1:9090\n\
                                secret: ''\n";
            let _ = std::fs::write(&config_path, default_yaml);
        }
    }

    pub fn start_core(&self) -> Result<String, String> {
        if !self.is_installed() {
            return Err(format!("Mihomo binary not found at {:?}", self.bin_path));
        }

        self.ensure_default_config();
        let log_path = self.data_dir.join("mihomo.log");
        let log_file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_path)
            .map_err(|e| format!("Failed to open log file: {}", e))?;

        let log_err = log_file
            .try_clone()
            .map_err(|e| format!("Failed to clone log handle: {}", e))?;

        #[cfg(target_os = "windows")]
        {
            use std::os::windows::process::CommandExt;
            const DETACHED_PROCESS: u32 = 0x00000008;
            const CREATE_NEW_PROCESS_GROUP: u32 = 0x00000200;
            const CREATE_NO_WINDOW: u32 = 0x08000000;

            let child = std::process::Command::new(&self.bin_path)
                .args(["-d", self.data_dir.to_str().unwrap_or("data")])
                .creation_flags(DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP | CREATE_NO_WINDOW)
                .stdout(log_file)
                .stderr(log_err)
                .spawn()
                .map_err(|e| format!("Failed to spawn core: {}", e))?;

            Ok(format!("Core started in background (PID {})", child.id()))
        }

        #[cfg(not(target_os = "windows"))]
        {
            #[cfg(unix)]
            use std::os::unix::process::CommandExt;

            let mut cmd = std::process::Command::new(&self.bin_path);
            cmd.args(["-d", self.data_dir.to_str().unwrap_or("data")])
                .stdout(log_file)
                .stderr(log_err);

            #[cfg(unix)]
            cmd.process_group(0);

            let child = cmd.spawn().map_err(|e| format!("Failed to spawn core: {}", e))?;
            Ok(format!("Core started in background (PID {})", child.id()))
        }
    }

    pub fn stop_core(&self) -> Result<String, String> {
        #[cfg(target_os = "windows")]
        {
            let output = std::process::Command::new("taskkill")
                .args(["/F", "/IM", "mihomo.exe"])
                .output()
                .map_err(|e| format!("Failed to execute taskkill: {}", e))?;

            if output.status.success() {
                Ok("Mihomo core stopped successfully".to_string())
            } else {
                Err("No running mihomo core found".to_string())
            }
        }

        #[cfg(not(target_os = "windows"))]
        {
            let output = std::process::Command::new("pkill")
                .args(["-f", "mihomo"])
                .output()
                .map_err(|e| format!("Failed to execute pkill: {}", e))?;

            if output.status.success() {
                Ok("Mihomo core stopped successfully".to_string())
            } else {
                Err("No running mihomo core found".to_string())
            }
        }
    }

    pub fn start_core_as_admin(&self) -> Result<String, String> {
        let _ = self.stop_core();
        std::thread::sleep(Duration::from_millis(500));

        #[cfg(target_os = "windows")]
        {
            let abs_bin = std::fs::canonicalize(&self.bin_path)
                .unwrap_or_else(|_| self.bin_path.clone());
            let abs_data = std::fs::canonicalize(&self.data_dir)
                .unwrap_or_else(|_| self.data_dir.clone());

            let cmd_str = format!(
                "Start-Process -FilePath '{}' -ArgumentList '-d', '{}' -Verb RunAs",
                abs_bin.display(),
                abs_data.display()
            );

            let output = std::process::Command::new("powershell")
                .args(["-Command", &cmd_str])
                .output()
                .map_err(|e| format!("Failed to launch elevated core: {}", e))?;

            if output.status.success() {
                Ok("Launched Mihomo core with Administrator privileges (UAC)".to_string())
            } else {
                Err("Administrator UAC elevation was canceled or failed".to_string())
            }
        }

        #[cfg(not(target_os = "windows"))]
        {
            Err("Admin elevation helper currently only configured for Windows (use sudo on macOS/Linux)".to_string())
        }
    }

    pub fn is_autostart_enabled(&self) -> bool {
        #[cfg(target_os = "macos")]
        {
            if let Some(home) = std::env::var_os("HOME") {
                let plist = PathBuf::from(home)
                    .join("Library/LaunchAgents/com.clash-tui.mihomo.plist");
                return plist.exists();
            }
            false
        }
        #[cfg(target_os = "windows")]
        {
            let output = std::process::Command::new("schtasks")
                .args(["/query", "/tn", "ClashTuiMihomo"])
                .output();
            matches!(output, Ok(o) if o.status.success())
        }
        #[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
        {
            if let Some(home) = std::env::var_os("HOME") {
                let service = PathBuf::from(home)
                    .join(".config/systemd/user/clash-tui-mihomo.service");
                return service.exists();
            }
            false
        }
    }

    pub fn enable_autostart(&self) -> Result<String, String> {
        let abs_bin = std::fs::canonicalize(&self.bin_path)
            .unwrap_or_else(|_| self.bin_path.clone());
        let abs_data = std::fs::canonicalize(&self.data_dir)
            .unwrap_or_else(|_| self.data_dir.clone());
        let log_file = abs_data.join("mihomo.log");

        #[cfg(target_os = "macos")]
        {
            let home = std::env::var("HOME").map_err(|e| format!("HOME not set: {}", e))?;
            let plist_dir = PathBuf::from(&home).join("Library/LaunchAgents");
            let _ = std::fs::create_dir_all(&plist_dir);
            let plist_file = plist_dir.join("com.clash-tui.mihomo.plist");

            let plist_content = format!(
                r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>com.clash-tui.mihomo</string>
    <key>ProgramArguments</key>
    <array>
        <string>{}</string>
        <string>-d</string>
        <string>{}</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <true/>
    <key>StandardOutPath</key>
    <string>{}</string>
    <key>StandardErrorPath</key>
    <string>{}</string>
    <key>ProcessType</key>
    <string>Background</string>
</dict>
</plist>
"#,
                abs_bin.display(),
                abs_data.display(),
                log_file.display(),
                log_file.display()
            );

            std::fs::write(&plist_file, plist_content)
                .map_err(|e| format!("Failed to write plist: {}", e))?;

            let _ = std::process::Command::new("launchctl")
                .args(["unload", "-w", plist_file.to_str().unwrap()])
                .output();

            let out = std::process::Command::new("launchctl")
                .args(["load", "-w", plist_file.to_str().unwrap()])
                .output()
                .map_err(|e| format!("Failed to run launchctl load: {}", e))?;

            if out.status.success() {
                Ok("Autostart service enabled (macOS LaunchAgents: com.clash-tui.mihomo)".to_string())
            } else {
                Err(format!("launchctl error: {}", String::from_utf8_lossy(&out.stderr)))
            }
        }

        #[cfg(target_os = "windows")]
        {
            let task_run = format!("\"{}\" -d \"{}\"", abs_bin.display(), abs_data.display());
            let output = std::process::Command::new("schtasks")
                .args(["/create", "/tn", "ClashTuiMihomo", "/tr", &task_run, "/sc", "onlogon", "/rl", "highest", "/f"])
                .output()
                .map_err(|e| format!("Failed to execute schtasks: {}", e))?;

            if output.status.success() {
                Ok("Autostart service enabled (Windows Scheduled Task: ClashTuiMihomo)".to_string())
            } else {
                Err("Failed to create scheduled task. Try running as Administrator.".to_string())
            }
        }

        #[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
        {
            let home = std::env::var("HOME").map_err(|e| format!("HOME not set: {}", e))?;
            let service_dir = PathBuf::from(&home).join(".config/systemd/user");
            let _ = std::fs::create_dir_all(&service_dir);
            let service_file = service_dir.join("clash-tui-mihomo.service");

            let service_content = format!(
                r#"[Unit]
Description=Clash TUI Mihomo Core Daemon
After=network.target

[Service]
Type=simple
ExecStart={} -d {}
Restart=always
RestartSec=3
StandardOutput=append:{}
StandardError=append:{}

[Install]
WantedBy=default.target
"#,
                abs_bin.display(),
                abs_data.display(),
                log_file.display(),
                log_file.display()
            );

            std::fs::write(&service_file, service_content)
                .map_err(|e| format!("Failed to write service file: {}", e))?;

            let _ = std::process::Command::new("systemctl")
                .args(["--user", "daemon-reload"])
                .output();

            let out = std::process::Command::new("systemctl")
                .args(["--user", "enable", "--now", "clash-tui-mihomo"])
                .output()
                .map_err(|e| format!("Failed to execute systemctl: {}", e))?;

            if out.status.success() {
                Ok("Autostart service enabled (systemd: clash-tui-mihomo)".to_string())
            } else {
                Err(format!("systemctl error: {}", String::from_utf8_lossy(&out.stderr)))
            }
        }
    }

    pub fn disable_autostart(&self) -> Result<String, String> {
        #[cfg(target_os = "macos")]
        {
            let home = std::env::var("HOME").map_err(|e| format!("HOME not set: {}", e))?;
            let plist_file = PathBuf::from(&home).join("Library/LaunchAgents/com.clash-tui.mihomo.plist");
            if plist_file.exists() {
                let _ = std::process::Command::new("launchctl")
                    .args(["unload", "-w", plist_file.to_str().unwrap()])
                    .output();
                let _ = std::fs::remove_file(&plist_file);
                Ok("Autostart service disabled (LaunchAgent removed)".to_string())
            } else {
                Ok("Autostart service is not currently enabled".to_string())
            }
        }

        #[cfg(target_os = "windows")]
        {
            let output = std::process::Command::new("schtasks")
                .args(["/delete", "/tn", "ClashTuiMihomo", "/f"])
                .output()
                .map_err(|e| format!("Failed to execute schtasks: {}", e))?;

            if output.status.success() {
                Ok("Autostart service disabled (Scheduled task removed)".to_string())
            } else {
                Ok("Autostart service is not currently enabled".to_string())
            }
        }

        #[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
        {
            let home = std::env::var("HOME").map_err(|e| format!("HOME not set: {}", e))?;
            let service_file = PathBuf::from(&home).join(".config/systemd/user/clash-tui-mihomo.service");
            if service_file.exists() {
                let _ = std::process::Command::new("systemctl")
                    .args(["--user", "disable", "--now", "clash-tui-mihomo"])
                    .output();
                let _ = std::fs::remove_file(&service_file);
                let _ = std::process::Command::new("systemctl")
                    .args(["--user", "daemon-reload"])
                    .output();
                Ok("Autostart service disabled (systemd service removed)".to_string())
            } else {
                Ok("Autostart service is not currently enabled".to_string())
            }
        }
    }
}
