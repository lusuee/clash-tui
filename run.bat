@echo off
chcp 65001 >nul
if exist "%~dp0target\release\clash-tui.exe" (
    "%~dp0target\release\clash-tui.exe" %*
) else (
    python "%~dp0clash_tui.py" %*
)
