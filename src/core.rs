use std::path::{Path, PathBuf};
use std::time::Duration;

pub struct CoreManager {
    pub bin_path: PathBuf,
    pub data_dir: PathBuf,
}

/// 去掉 Windows `\\?\` 长路径前缀（提示信息与内核路径都更干净，也避免拼进日志里的非法路径）。
fn strip_verbatim(path: PathBuf) -> PathBuf {
    let s = path.to_string_lossy().to_string();
    let s = if let Some(rest) = s.strip_prefix(r"\\?\UNC\") {
        format!(r"\\{rest}")
    } else if let Some(rest) = s.strip_prefix(r"\\?\") {
        rest.to_string()
    } else {
        s
    };
    PathBuf::from(s)
}

impl CoreManager {
    #[cfg(target_os = "windows")]
    const BIN_NAME: &'static str = "mihomo.exe";
    #[cfg(not(target_os = "windows"))]
    const BIN_NAME: &'static str = "mihomo";

    pub fn new() -> Self {
        let bin_name = Self::BIN_NAME;

        // 优先使用当前工作目录（便携布局 / 开发目录）中的内核；
        // 若 cwd 中没有，则回退到 clash-tui 可执行文件旁边的 bin/：
        // cwd 只是用户随手打开终端的目录时，不应因此误报“找不到 mihomo”。
        let primary_path = Path::new("bin").join(bin_name);
        let direct_path = PathBuf::from(bin_name);
        let nested_path = Path::new("bin").join("mihomo").join(bin_name);
        let bin_path = if primary_path.is_file() {
            primary_path
        } else if direct_path.is_file() {
            direct_path
        } else if nested_path.is_file() {
            nested_path
        } else {
            Self::kernel_next_to_exe().unwrap_or(primary_path)
        };
        let data_dir = PathBuf::from("data");

        let _ = std::fs::create_dir_all("bin");
        let _ = std::fs::create_dir_all(&data_dir);

        Self { bin_path, data_dir }
    }

    pub fn is_installed(&self) -> bool {
        self.bin_path.is_file()
    }

    /// 是否仍有 mihomo 进程存在。
    ///
    /// 直接查进程而不是 REST：更新内核时必须确认旧进程真的退出了，
    /// REST 探测在 secret 不匹配/内核以管理员身份启动时可能不可靠。
    pub fn process_alive() -> bool {
        #[cfg(target_os = "windows")]
        {
            let out = std::process::Command::new("tasklist")
                .args(["/FI", "IMAGENAME eq mihomo.exe", "/NH"])
                .output();
            match out {
                Ok(o) => String::from_utf8_lossy(&o.stdout).lines().any(|line| {
                    line.trim_start().to_ascii_lowercase().starts_with("mihomo.exe")
                }),
                Err(_) => false,
            }
        }
        #[cfg(not(target_os = "windows"))]
        {
            std::process::Command::new("pkill")
                .args(["-0", "-f", "mihomo"])
                .status()
                .map(|s| s.success())
                .unwrap_or(false)
        }
    }

    /// 在 clash-tui 可执行文件所在目录中查找内核（`mihomo`、`bin/mihomo`、`bin/mihomo/mihomo`）。
    /// 返回绝对路径，供 cwd 中缺少内核时兜底使用。
    pub fn kernel_next_to_exe() -> Option<PathBuf> {
        let exe = std::env::current_exe().ok()?;
        let exe = strip_verbatim(std::fs::canonicalize(&exe).unwrap_or(exe));
        let dir = exe.parent()?;
        [
            dir.join(Self::BIN_NAME),
            dir.join("bin").join(Self::BIN_NAME),
            dir.join("bin").join("mihomo").join(Self::BIN_NAME),
        ]
        .into_iter()
        .find(|p| p.is_file())
    }

    /// 内核缺失时的可操作提示：告诉用户实际查找过的位置。
    pub fn missing_hint(&self) -> String {
        let exe_dir = std::env::current_exe()
            .ok()
            .map(|p| strip_verbatim(std::fs::canonicalize(&p).unwrap_or(p)))
            .and_then(|p| p.parent().map(|d| d.to_path_buf()));
        let install_dir = exe_dir
            .map(|d| d.display().to_string())
            .unwrap_or_else(|| "<安装目录>".to_string());
        format!(
            "未找到 Mihomo 内核（已查找 {} 与 {}/bin/{}）；请将内核放到 {}/bin/{} 并执行 chmod +x",
            self.bin_path.display(),
            install_dir,
            Self::BIN_NAME,
            install_dir,
            Self::BIN_NAME
        )
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
                                secret: ''\n\
                                tun:\n\
                                  enable: false\n";
            let _ = std::fs::write(&config_path, default_yaml);
        } else {
            Self::ensure_tun_disabled_in_config(&config_path);
        }
    }

    pub fn ensure_tun_disabled_in_config(config_path: &Path) {
        if !config_path.exists() {
            return;
        }
        if let Ok(content) = std::fs::read_to_string(config_path) {
            if let Ok(mut yaml_val) = serde_yaml::from_str::<serde_yaml::Value>(&content) {
                let mut modified = false;
                if let Some(map) = yaml_val.as_mapping_mut() {
                    let tun_key = serde_yaml::Value::String("tun".to_string());
                    if let Some(tun_val) = map.get_mut(&tun_key) {
                        if let Some(tun_map) = tun_val.as_mapping_mut() {
                            let enable_key = serde_yaml::Value::String("enable".to_string());
                            if let Some(enable_val) = tun_map.get_mut(&enable_key) {
                                if enable_val.as_bool() != Some(false) {
                                    *enable_val = serde_yaml::Value::Bool(false);
                                    modified = true;
                                }
                            } else {
                                tun_map.insert(enable_key, serde_yaml::Value::Bool(false));
                                modified = true;
                            }
                        }
                    } else {
                        let mut tun_map = serde_yaml::Mapping::new();
                        tun_map.insert(
                            serde_yaml::Value::String("enable".to_string()),
                            serde_yaml::Value::Bool(false),
                        );
                        map.insert(tun_key, serde_yaml::Value::Mapping(tun_map));
                        modified = true;
                    }
                }
                if modified {
                    if let Ok(out) = serde_yaml::to_string(&yaml_val) {
                        let _ = std::fs::write(config_path, out);
                        crate::rules::apply_rules_to_config();
                    }
                }
            }
        }
    }

    pub fn start_core(&self) -> Result<String, String> {
        if !self.is_installed() {
            return Err(self.missing_hint());
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
        self.ensure_default_config();

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
        #[allow(unused_variables)]
        let abs_bin = std::fs::canonicalize(&self.bin_path)
            .unwrap_or_else(|_| self.bin_path.clone());
        #[allow(unused_variables)]
        let abs_data = std::fs::canonicalize(&self.data_dir)
            .unwrap_or_else(|_| self.data_dir.clone());
        #[allow(unused_variables)]
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
            use base64::Engine;

            if !self.bin_path.exists() {
                return Err(format!(
                    "Mihomo binary not found at {:?}. Please place mihomo.exe in the 'bin' directory.",
                    self.bin_path
                ));
            }

            self.ensure_default_config();

            let abs_bin = std::fs::canonicalize(&self.bin_path).unwrap_or_else(|_| self.bin_path.clone());
            let abs_data = std::fs::canonicalize(&self.data_dir).unwrap_or_else(|_| self.data_dir.clone());
            let clean_bin = clean_windows_path(&abs_bin);
            let clean_data = clean_windows_path(&abs_data);

            // Write silent_start.vbs to launch Mihomo completely hidden (no console black window on boot)
            let vbs_path = self.bin_path.parent().unwrap_or(Path::new("bin")).join("silent_start.vbs");
            let vbs_content = format!(
                "Dim WshShell\r\nSet WshShell = CreateObject(\"WScript.Shell\")\r\nWshShell.Run chr(34) & \"{}\" & chr(34) & \" -d \" & chr(34) & \"{}\" & chr(34), 0, False\r\n",
                clean_bin,
                clean_data
            );
            // Write strictly as ASCII bytes without UTF-8 BOM to prevent Windows Script Host error 800A0408
            std::fs::write(&vbs_path, vbs_content.as_bytes())
                .map_err(|e| format!("Failed to write silent_start.vbs: {}", e))?;

            let abs_vbs = std::fs::canonicalize(&vbs_path).unwrap_or_else(|_| vbs_path.clone());
            let clean_vbs = clean_windows_path(&abs_vbs);
            let work_dir = clean_windows_path(std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from(".")));

            // Use PowerShell ScheduledTask cmdlets encoded in base64 to avoid quoting issues
            let ps_script = format!(
                "$Action = New-ScheduledTaskAction -Execute 'wscript.exe' -Argument '\"{}\"' -WorkingDirectory '{}'\n\
                 $Trigger = New-ScheduledTaskTrigger -AtLogOn\n\
                 $Principal = New-ScheduledTaskPrincipal -UserId $env:USERNAME -LogonType Interactive -RunLevel Highest\n\
                 $Settings = New-ScheduledTaskSettingsSet -AllowStartIfOnBatteries -DontStopIfGoingOnBatteries\n\
                 Register-ScheduledTask -TaskName 'ClashTuiMihomo' -Action $Action -Trigger $Trigger -Principal $Principal -Settings $Settings -Force",
                clean_vbs,
                work_dir
            );

            let utf16: Vec<u8> = ps_script.encode_utf16().flat_map(|u| u.to_le_bytes()).collect();
            let b64 = base64::engine::general_purpose::STANDARD.encode(&utf16);

            // 1. Try directly (works if already running as Admin)
            let direct_out = std::process::Command::new("powershell")
                .args(["-NoProfile", "-EncodedCommand", &b64])
                .output();

            if let Ok(ref o) = direct_out {
                if o.status.success() && self.is_autostart_enabled() {
                    return Ok("Autostart scheduled task created (Windows: ClashTuiMihomo, silent background mode)".to_string());
                }
            }

            // 2. If non-admin, request UAC elevation
            println!("[提示] 注册最高权限计划任务需要管理员权限，正在请求 UAC 提权...");
            let elevate_cmd = format!(
                "Start-Process powershell -ArgumentList '-NoProfile', '-EncodedCommand', '{}' -Verb RunAs -Wait",
                b64
            );
            let _ = std::process::Command::new("powershell")
                .args(["-NoProfile", "-Command", &elevate_cmd])
                .output();

            if self.is_autostart_enabled() {
                Ok("Autostart scheduled task created via UAC elevation (ClashTuiMihomo, silent background mode)".to_string())
            } else {
                Err("Failed to create scheduled task. Administrator permission was denied or failed.".to_string())
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
                .output();

            let success = match output {
                Ok(ref o) => o.status.success(),
                Err(_) => false,
            };

            if !success {
                let ps_cmd = "Start-Process schtasks -ArgumentList '/delete /tn ClashTuiMihomo /f' -Verb RunAs -Wait";
                let _ = std::process::Command::new("powershell")
                    .args(["-NoProfile", "-ExecutionPolicy", "Bypass", "-Command", ps_cmd])
                    .output();
            }

            let vbs_path = self.bin_path.parent().unwrap_or(Path::new("bin")).join("silent_start.vbs");
            if vbs_path.exists() {
                let _ = std::fs::remove_file(vbs_path);
            }

            if !self.is_autostart_enabled() {
                Ok("Autostart scheduled task removed (ClashTuiMihomo)".to_string())
            } else {
                Err("Failed to remove scheduled task. Administrator permission was denied or failed.".to_string())
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

#[cfg(target_os = "windows")]
fn clean_windows_path<P: AsRef<std::path::Path>>(path: P) -> String {
    let s = path.as_ref().to_string_lossy().to_string();
    if let Some(stripped) = s.strip_prefix(r"\\?\") {
        stripped.to_string()
    } else {
        s
    }
}

