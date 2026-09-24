#[cfg(target_os = "windows")]
use winreg::enums::*;
#[cfg(target_os = "windows")]
use winreg::RegKey;

#[cfg(target_os = "windows")]
mod wininet {
    const INTERNET_OPTION_SETTINGS_CHANGED: u32 = 39;
    const INTERNET_OPTION_REFRESH: u32 = 37;

    #[link(name = "wininet")]
    extern "system" {
        fn InternetSetOptionW(
            h_internet: *mut std::ffi::c_void,
            dw_option: u32,
            lp_buffer: *mut std::ffi::c_void,
            dw_buffer_length: u32,
        ) -> i32;
    }

    pub fn notify_system_proxy_change() {
        unsafe {
            InternetSetOptionW(
                std::ptr::null_mut(),
                INTERNET_OPTION_SETTINGS_CHANGED,
                std::ptr::null_mut(),
                0,
            );
            InternetSetOptionW(
                std::ptr::null_mut(),
                INTERNET_OPTION_REFRESH,
                std::ptr::null_mut(),
                0,
            );
        }
    }
}

pub struct SysProxy;

impl SysProxy {
    // ------------------------------------------------------------------------
    // Windows Implementation
    // ------------------------------------------------------------------------
    #[cfg(target_os = "windows")]
    pub fn get_status() -> Result<(bool, String), String> {
        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        let settings = hkcu
            .open_subkey("Software\\Microsoft\\Windows\\CurrentVersion\\Internet Settings")
            .map_err(|e| e.to_string())?;

        let enabled: u32 = settings.get_value("ProxyEnable").unwrap_or(0);
        let server: String = settings.get_value("ProxyServer").unwrap_or_default();

        Ok((enabled != 0, server))
    }

    #[cfg(target_os = "windows")]
    pub fn set_proxy(enable: bool, server: &str) -> Result<(), String> {
        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        let (settings, _) = hkcu
            .create_subkey("Software\\Microsoft\\Windows\\CurrentVersion\\Internet Settings")
            .map_err(|e| e.to_string())?;

        let enable_val: u32 = if enable { 1 } else { 0 };
        settings
            .set_value("ProxyEnable", &enable_val)
            .map_err(|e| e.to_string())?;

        if enable && !server.is_empty() {
            settings
                .set_value("ProxyServer", &server)
                .map_err(|e| e.to_string())?;
        }

        wininet::notify_system_proxy_change();
        Ok(())
    }

    // ------------------------------------------------------------------------
    // macOS Implementation (via networksetup)
    // ------------------------------------------------------------------------
    #[cfg(target_os = "macos")]
    fn get_macos_services() -> Vec<String> {
        let output = std::process::Command::new("networksetup")
            .arg("-listallnetworkservices")
            .output();

        if let Ok(out) = output {
            let stdout = String::from_utf8_lossy(&out.stdout);
            stdout
                .lines()
                .filter(|line| !line.contains('*') && !line.trim().is_empty())
                .map(|s| s.trim().to_string())
                .collect()
        } else {
            vec!["Wi-Fi".to_string(), "Ethernet".to_string()]
        }
    }

    #[cfg(target_os = "macos")]
    pub fn get_status() -> Result<(bool, String), String> {
        let services = Self::get_macos_services();
        for svc in services {
            let output = std::process::Command::new("networksetup")
                .args(["-getwebproxy", &svc])
                .output();

            if let Ok(out) = output {
                let text = String::from_utf8_lossy(&out.stdout);
                let mut enabled = false;
                let mut server = String::new();
                let mut port = String::new();

                for line in text.lines() {
                    let parts: Vec<&str> = line.split(':').map(|s| s.trim()).collect();
                    if parts.len() >= 2 {
                        if parts[0] == "Enabled" && parts[1].eq_ignore_ascii_case("yes") {
                            enabled = true;
                        } else if parts[0] == "Server" {
                            server = parts[1].to_string();
                        } else if parts[0] == "Port" {
                            port = parts[1].to_string();
                        }
                    }
                }

                if enabled {
                    let addr = if !port.is_empty() && port != "0" {
                        format!("{}:{}", server, port)
                    } else {
                        server
                    };
                    return Ok((true, addr));
                }
            }
        }
        Ok((false, String::new()))
    }

    #[cfg(target_os = "macos")]
    pub fn set_proxy(enable: bool, server: &str) -> Result<(), String> {
        let services = Self::get_macos_services();
        let (host, port) = if let Some((h, p)) = server.split_once(':') {
            (h, p)
        } else {
            ("127.0.0.1", "7897")
        };

        for svc in services {
            if enable {
                let _ = std::process::Command::new("networksetup")
                    .args(["-setwebproxy", &svc, host, port])
                    .status();
                let _ = std::process::Command::new("networksetup")
                    .args(["-setsecurewebproxy", &svc, host, port])
                    .status();
                let _ = std::process::Command::new("networksetup")
                    .args(["-setsocksfirewallproxy", &svc, host, port])
                    .status();
                let _ = std::process::Command::new("networksetup")
                    .args(["-setwebproxystate", &svc, "on"])
                    .status();
                let _ = std::process::Command::new("networksetup")
                    .args(["-setsecurewebproxystate", &svc, "on"])
                    .status();
                let _ = std::process::Command::new("networksetup")
                    .args(["-setsocksfirewallproxystate", &svc, "on"])
                    .status();
            } else {
                let _ = std::process::Command::new("networksetup")
                    .args(["-setwebproxystate", &svc, "off"])
                    .status();
                let _ = std::process::Command::new("networksetup")
                    .args(["-setsecurewebproxystate", &svc, "off"])
                    .status();
                let _ = std::process::Command::new("networksetup")
                    .args(["-setsocksfirewallproxystate", &svc, "off"])
                    .status();
            }
        }
        Ok(())
    }

    // ------------------------------------------------------------------------
    // Fallback for Linux or other targets
    // ------------------------------------------------------------------------
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    pub fn get_status() -> Result<(bool, String), String> {
        Ok((false, String::new()))
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    pub fn set_proxy(_enable: bool, _server: &str) -> Result<(), String> {
        Ok(())
    }
}
