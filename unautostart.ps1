# Clash TUI Windows 取消开机自启脚本 (PowerShell)
[Console]::OutputEncoding = [System.Text.Encoding]::UTF8

$isAdmin = ([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
if (-not $isAdmin) {
    Write-Host "[提示] 正在请求管理员权限以注销开机自启任务..." -ForegroundColor Yellow
    Start-Process powershell -Verb RunAs -ArgumentList "-NoProfile -ExecutionPolicy Bypass -File `"$PSCommandPath`""
    exit
}

try {
    Unregister-ScheduledTask -TaskName "ClashTuiMihomo" -Confirm:$false -ErrorAction Stop
    Write-Host ""
    Write-Host "[成功] 已注销开机自启任务 (ClashTuiMihomo)。" -ForegroundColor Green
} catch {
    Write-Host ""
    Write-Host "[提示] 任务不存在或已注销。" -ForegroundColor Yellow
}

Write-Host ""
Read-Host "按回车键退出"
