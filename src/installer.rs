#[cfg(not(target_os = "windows"))]
use std::path::PathBuf;

pub fn install(autostart: bool) -> Result<String, String> {
    let exe_path = std::env::current_exe().map_err(|e| format!("Failed to get current executable path: {}", e))?;
    let exe_dir = exe_path.parent().ok_or("Cannot determine parent directory of executable")?;
    let exe_dir_str = exe_dir.to_str().ok_or("Invalid UTF-8 in executable path")?;

    #[cfg(target_os = "windows")]
    {
        use winreg::enums::*;
        use winreg::RegKey;

        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        let (env_key, _) = hkcu
            .create_subkey("Environment")
            .map_err(|e| format!("Failed to open HKCU\\Environment: {}", e))?;

        let current_path: String = env_key.get_value("Path").unwrap_or_default();
        let normalized_target = exe_dir_str.trim_end_matches('\\').to_lowercase();

        let entries: Vec<&str> = current_path
            .split(';')
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .collect();

        let already_in_path = entries.iter().any(|&e| {
            e.trim_end_matches('\\').to_lowercase() == normalized_target
        });

        let mut output_msgs = Vec::new();

        if already_in_path {
            output_msgs.push(format!("\"{}\" is already in User PATH.", exe_dir_str));
        } else {
            let new_path = if current_path.is_empty() {
                exe_dir_str.to_string()
            } else if current_path.ends_with(';') {
                format!("{}{}", current_path, exe_dir_str)
            } else {
                format!("{};{}", current_path, exe_dir_str)
            };

            env_key
                .set_value("Path", &new_path)
                .map_err(|e| format!("Failed to update Path in registry: {}", e))?;

            crate::envproxy::winuser::notify_environment_change();
            output_msgs.push(format!("Added \"{}\" to User PATH.", exe_dir_str));
            output_msgs.push("Open a new terminal to run 'clash-tui' anywhere.".to_string());
        }

        if autostart {
            let core_mgr = crate::core::CoreManager::new();
            match core_mgr.enable_autostart() {
                Ok(msg) => output_msgs.push(format!("✔ {}", msg)),
                Err(e) => output_msgs.push(format!("✘ Autostart configuration failed: {}", e)),
            }
        }

        Ok(output_msgs.join("\n"))
    }

    #[cfg(not(target_os = "windows"))]
    {
        let home = std::env::var("HOME").map_err(|e| format!("HOME not set: {}", e))?;
        let local_bin = PathBuf::from(&home).join(".local/bin");
        let _ = std::fs::create_dir_all(&local_bin);
        let symlink_path = local_bin.join("clash-tui");

        if symlink_path.exists() || symlink_path.is_symlink() {
            let _ = std::fs::remove_file(&symlink_path);
        }

        #[cfg(unix)]
        std::os::unix::fs::symlink(&exe_path, &symlink_path)
            .map_err(|e| format!("Failed to create symlink: {}", e))?;

        let mut output_msgs = Vec::new();
        output_msgs.push(format!("Created symlink at: {}", symlink_path.display()));

        let path_env = std::env::var("PATH").unwrap_or_default();
        let local_bin_str = local_bin.to_str().unwrap_or("");
        let in_path = path_env.split(':').any(|p| p == local_bin_str);

        if in_path {
            output_msgs.push("You can now run 'clash-tui' anywhere in your terminal.".to_string());
        } else {
            output_msgs.push(format!(
                "Note: ~/.local/bin is not in your current PATH. Add it to ~/.zshrc or ~/.bashrc:\n    export PATH=\"$HOME/.local/bin:$PATH\""
            ));
        }

        if autostart {
            let core_mgr = crate::core::CoreManager::new();
            match core_mgr.enable_autostart() {
                Ok(msg) => output_msgs.push(format!("✔ {}", msg)),
                Err(e) => output_msgs.push(format!("✘ Autostart configuration failed: {}", e)),
            }
        }

        Ok(output_msgs.join("\n"))
    }
}

pub fn uninstall() -> Result<String, String> {
    let exe_path = std::env::current_exe().map_err(|e| format!("Failed to get current executable path: {}", e))?;
    let exe_dir = exe_path.parent().ok_or("Cannot determine parent directory of executable")?;
    let exe_dir_str = exe_dir.to_str().ok_or("Invalid UTF-8 in executable path")?;

    let mut output_msgs = Vec::new();

    #[cfg(target_os = "windows")]
    {
        use winreg::enums::*;
        use winreg::RegKey;

        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        if let Ok(env_key) = hkcu.open_subkey_with_flags("Environment", KEY_READ | KEY_WRITE) {
            let current_path: String = env_key.get_value("Path").unwrap_or_default();
            let normalized_target = exe_dir_str.trim_end_matches('\\').to_lowercase();

            let remaining: Vec<&str> = current_path
                .split(';')
                .map(|s| s.trim())
                .filter(|s| !s.is_empty() && s.trim_end_matches('\\').to_lowercase() != normalized_target)
                .collect();

            let new_path = remaining.join(";");
            let _ = env_key.set_value("Path", &new_path);
            crate::envproxy::winuser::notify_environment_change();
            output_msgs.push(format!("Removed \"{}\" from User PATH.", exe_dir_str));
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        let home = std::env::var("HOME").unwrap_or_default();
        let symlink_path = PathBuf::from(home).join(".local/bin/clash-tui");
        if symlink_path.exists() || symlink_path.is_symlink() {
            let _ = std::fs::remove_file(&symlink_path);
            output_msgs.push(format!("Removed symlink: {}", symlink_path.display()));
        }
    }

    // Disable autostart if active
    let core_mgr = crate::core::CoreManager::new();
    if core_mgr.is_autostart_enabled() {
        match core_mgr.disable_autostart() {
            Ok(msg) => output_msgs.push(format!("✔ {}", msg)),
            Err(e) => output_msgs.push(format!("✘ Autostart removal failed: {}", e)),
        }
    }

    if output_msgs.is_empty() {
        Ok("Uninstall completed (no active configurations found).".to_string())
    } else {
        Ok(output_msgs.join("\n"))
    }
}
