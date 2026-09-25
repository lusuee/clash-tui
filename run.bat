@echo off
chcp 65001 >nul
if exist "%~dp0clash-tui.exe" (
    "%~dp0clash-tui.exe" %*
) else if exist "%~dp0target\release\clash-tui.exe" (
    "%~dp0target\release\clash-tui.exe" %*
) else (
    echo [ERROR] clash-tui.exe not found. Run: cargo build --release
)
