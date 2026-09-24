use std::fs;
#[cfg(target_os = "macos")]
use std::path::Path;

#[cfg(target_os = "windows")]
use winreg::enums::*;
#[cfg(target_os = "windows")]
use winreg::RegKey;

#[cfg(target_os = "windows")]
mod winuser {
    const HWND_BROADCAST: *mut std::ffi::c_void = 0xffff as *mut std::ffi::c_void;
    const WM_SETTINGCHANGE: u32 = 0x001a;
    const SMTO_ABORTIFHUNG: u32 = 0x0002;

    #[link(name = "user32")]
    extern "system" {
        fn SendMessageTimeoutW(
            hwnd: *mut std::ffi::c_void,
            msg: u32,
            w_param: usize,
            l_param: *const u16,
            fu_flags: u32,
            u_timeout: u32,
            lpdw_result: *mut usize,
        ) -> isize;
    }

    pub fn notify_environment_change() {
        let env_wide: Vec<u16> = "Environment\0".encode_utf16().collect();
        let mut result: usize = 0;
        unsafe {
            SendMessageTimeoutW(
                HWND_BROADCAST,
                WM_SETTINGCHANGE,
                0,
                env_wide.as_ptr(),
                SMTO_ABORTIFHUNG,
                1000,
                &mut result,
            );
        }
    }
}

pub struct EnvProxy;

impl EnvProxy {
    /// Check whether environment variable proxy is currently set
    pub fn get_status() -> bool {
        #[cfg(target_os = "windows")]
        {
            let hkcu = RegKey::predef(HKEY_CURRENT_USER);
            if let Ok(env_key) = hkcu.open_subkey("Environment") {
                let http: String = env_key.get_value("http_proxy").unwrap_or_default();
                return !http.is_empty();
            }
            false
        }

        #[cfg(target_os = "macos")]
        {
            let home = std::env::var("HOME").unwrap_or_default();
            Path::new(&home).join(".clash_env.sh").exists()
        }

        #[cfg(not(any(target_os = "windows", target_os = "macos")))]
        {
            false
        }
    }

    /// Toggle or set environment variable proxy
    pub fn set_env_proxy(enable: bool, port: u16) -> Result<(), String> {
        let http_val = format!("http://127.0.0.1:{}", port);
        let socks_val = format!("socks5://127.0.0.1:{}", port);

        #[cfg(target_os = "windows")]
        {
            let hkcu = RegKey::predef(HKEY_CURRENT_USER);
            let (env_key, _) = hkcu
                .create_subkey("Environment")
                .map_err(|e| format!("Failed to open HKCU\\Environment: {}", e))?;

            let keys = [
                "http_proxy",
                "https_proxy",
                "all_proxy",
                "HTTP_PROXY",
                "HTTPS_PROXY",
                "ALL_PROXY",
            ];

            if enable {
                let _ = env_key.set_value("http_proxy", &http_val);
                let _ = env_key.set_value("https_proxy", &http_val);
                let _ = env_key.set_value("all_proxy", &socks_val);
                let _ = env_key.set_value("HTTP_PROXY", &http_val);
                let _ = env_key.set_value("HTTPS_PROXY", &http_val);
                let _ = env_key.set_value("ALL_PROXY", &socks_val);
            } else {
                for key in &keys {
                    let _ = env_key.delete_value(key);
                }
            }

            winuser::notify_environment_change();
        }

        #[cfg(target_os = "macos")]
        {
            let home = std::env::var("HOME").unwrap_or_default();
            let env_file = Path::new(&home).join(".clash_env.sh");

            if enable {
                let content = format!(
                    "export http_proxy=\"{}\"\nexport https_proxy=\"{}\"\nexport all_proxy=\"{}\"\nexport HTTP_PROXY=\"{}\"\nexport HTTPS_PROXY=\"{}\"\nexport ALL_PROXY=\"{}\"\n",
                    http_val, http_val, socks_val, http_val, http_val, socks_val
                );
                let _ = fs::write(&env_file, content);

                // Try launchctl for system-level session propagation
                let _ = std::process::Command::new("launchctl")
                    .args(&["setenv", "http_proxy", &http_val])
                    .output();
                let _ = std::process::Command::new("launchctl")
                    .args(&["setenv", "https_proxy", &http_val])
                    .output();
                let _ = std::process::Command::new("launchctl")
                    .args(&["setenv", "all_proxy", &socks_val])
                    .output();
            } else {
                let _ = fs::remove_file(&env_file);
                let _ = std::process::Command::new("launchctl").args(&["unsetenv", "http_proxy"]).output();
                let _ = std::process::Command::new("launchctl").args(&["unsetenv", "https_proxy"]).output();
                let _ = std::process::Command::new("launchctl").args(&["unsetenv", "all_proxy"]).output();
            }
        }

        // Always update / generate helper shell scripts for current directory
        Self::generate_shell_scripts(port);

        Ok(())
    }

    /// Generate instant helper scripts for Nushell, CMD, PowerShell, Bash
    pub fn generate_shell_scripts(port: u16) {
        let http_val = format!("http://127.0.0.1:{}", port);
        let socks_val = format!("socks5://127.0.0.1:{}", port);

        // 1. Nushell (env.nu / unenv.nu)
        let nu_content = format!(
            "# Load Clash Proxy into Nushell\n$env.http_proxy = \"{}\"\n$env.https_proxy = \"{}\"\n$env.all_proxy = \"{}\"\n$env.HTTP_PROXY = \"{}\"\n$env.HTTPS_PROXY = \"{}\"\n$env.ALL_PROXY = \"{}\"\nprint $\"✔ Proxy env loaded ({})\"\n",
            http_val, http_val, socks_val, http_val, http_val, socks_val, port
        );
        let _ = fs::write("env.nu", nu_content);

        let un_nu_content = "# Unset Clash Proxy in Nushell\nfor v in [\"http_proxy\", \"https_proxy\", \"all_proxy\", \"HTTP_PROXY\", \"HTTPS_PROXY\", \"ALL_PROXY\"] {\n    if $v in $env {\n        hide-env $v\n    }\n}\nprint \"✔ Proxy env cleared\"\n";
        let _ = fs::write("unenv.nu", un_nu_content);

        // 2. Windows Batch (env.bat / unenv.bat)
        let bat_content = format!(
            "@echo off\nset http_proxy={}\nset https_proxy={}\nset all_proxy={}\nset HTTP_PROXY={}\nset HTTPS_PROXY={}\nset ALL_PROXY={}\necho [OK] Proxy env set to {}\n",
            http_val, http_val, socks_val, http_val, http_val, socks_val, port
        );
        let _ = fs::write("env.bat", bat_content);

        let un_bat_content = "@echo off\nset http_proxy=\nset https_proxy=\nset all_proxy=\nset HTTP_PROXY=\nset HTTPS_PROXY=\nset ALL_PROXY=\necho [OK] Proxy env cleared\n";
        let _ = fs::write("unenv.bat", un_bat_content);

        // 3. PowerShell (env.ps1 / unenv.ps1)
        let ps_content = format!(
            "$env:http_proxy = \"{}\"\n$env:https_proxy = \"{}\"\n$env:all_proxy = \"{}\"\n$env:HTTP_PROXY = \"{}\"\n$env:HTTPS_PROXY = \"{}\"\n$env:ALL_PROXY = \"{}\"\nWrite-Host \"✔ Proxy env set to {}\"\n",
            http_val, http_val, socks_val, http_val, http_val, socks_val, port
        );
        let _ = fs::write("env.ps1", ps_content);

        let un_ps_content = "$env:http_proxy = $null\n$env:https_proxy = $null\n$env:all_proxy = $null\n$env:HTTP_PROXY = $null\n$env:HTTPS_PROXY = $null\n$env:ALL_PROXY = $null\nWrite-Host \"✔ Proxy env cleared\"\n";
        let _ = fs::write("unenv.ps1", un_ps_content);

        // 4. POSIX Shell (env.sh / unenv.sh)
        let sh_content = format!(
            "export http_proxy=\"{}\"\nexport https_proxy=\"{}\"\nexport all_proxy=\"{}\"\nexport HTTP_PROXY=\"{}\"\nexport HTTPS_PROXY=\"{}\"\nexport ALL_PROXY=\"{}\"\necho \"✔ Proxy env set to {}\"\n",
            http_val, http_val, socks_val, http_val, http_val, socks_val, port
        );
        let _ = fs::write("env.sh", sh_content);

        let un_sh_content = "unset http_proxy https_proxy all_proxy HTTP_PROXY HTTPS_PROXY ALL_PROXY\necho \"✔ Proxy env cleared\"\n";
        let _ = fs::write("unenv.sh", un_sh_content);
    }
}
