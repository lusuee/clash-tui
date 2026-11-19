//! 把版本号注入编译单元：`CLASH_TUI_VERSION`。
//!
//! 优先级：
//! 1. 构建环境变量 `CLASH_TUI_VERSION`（CI 在 tag 构建时传 `github.ref_name`，
//!    例如 `v0.2.5`，保证发布出来的二进制自带真实版本）；
//! 2. 本地 `git describe --tags --dirty`（有 tag 时形如 `v0.2.5` 或 `v0.2.5-1-g4d22d4b-dirty`）；
//! 3. 最后回退到 `CARGO_PKG_VERSION`。
//!
//! 自更新的版本比较只接受纯 `vX.Y.Z`，带 `-dirty`/`-N-gsha` 后缀的本地构建
//! 会被视为开发版，不参与自动升级。

use std::process::Command;

fn git_describe() -> Option<String> {
    let out = Command::new("git")
        .args(["describe", "--tags", "--dirty", "--always"])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if text.is_empty() {
        return None;
    }
    // 没有任何 tag 时 git describe --always 只给一个裸 commit sha，对版本没有意义。
    if text.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    Some(text)
}

fn main() {
    println!("cargo:rerun-if-env-changed=CLASH_TUI_VERSION");
    println!("cargo:rerun-if-changed=.git/HEAD");

    let version = std::env::var("CLASH_TUI_VERSION")
        .ok()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
        .or_else(git_describe)
        .unwrap_or_else(|| {
            std::env::var("CARGO_PKG_VERSION").unwrap_or_else(|_| "unknown".to_string())
        });

    println!("cargo:rustc-env=CLASH_TUI_VERSION={version}");
}