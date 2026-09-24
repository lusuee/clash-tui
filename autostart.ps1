# Clash TUI Windows 开机自启注册脚本 (PowerShell)
[Console]::OutputEncoding = [System.Text.Encoding]::UTF8

Write-Host "=========================================================" -ForegroundColor Cyan
Write-Host "  Clash TUI - Windows 开机自启服务注册程序" -ForegroundColor Cyan
Write-Host "=========================================================" -ForegroundColor Cyan
Write-Host ""

# 1. 检查管理员权限
$isAdmin = ([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
if (-not $isAdmin) {
    Write-Host "[提示] 正在请求管理员权限以注册系统计划任务..." -ForegroundColor Yellow
    Start-Process powershell -Verb RunAs -ArgumentList "-NoProfile -ExecutionPolicy Bypass -File `"$PSCommandPath`""
    exit
}

# 2. 定位内核与数据目录
$ScriptDir = Split-Path -Parent $PSCommandPath
$CoreExe = Join-Path $ScriptDir "bin\mihomo.exe"
$DataDir = Join-Path $ScriptDir "data"
$WorkDir = $ScriptDir

if (-not (Test-Path $CoreExe)) {
    $GlobalExe = "$env:LOCALAPPDATA\clash-tui\bin\mihomo.exe"
    if (Test-Path $GlobalExe) {
        $CoreExe = $GlobalExe
        $DataDir = "$env:LOCALAPPDATA\clash-tui\data"
        $WorkDir = "$env:LOCALAPPDATA\clash-tui"
    } else {
        Write-Host "[错误] 未找到 mihomo.exe 内核文件！" -ForegroundColor Red
        Write-Host "请确认 bin\mihomo.exe 存在或已运行 install.bat 安装。" -ForegroundColor Red
        Write-Host ""
        Read-Host "按回车键退出"
        exit 1
    }
}

Write-Host "[1/3] 核心文件定位:" -ForegroundColor Green
Write-Host "      内核文件: $CoreExe"
Write-Host "      数据目录: $DataDir"
Write-Host "      工作目录: $WorkDir"
Write-Host ""

# 3. 生成后台静默启动 VBS 脚本 (彻底消除开机黑框)
$VbsPath = Join-Path $WorkDir "bin\silent_start.vbs"
$VbsDir = Split-Path -Parent $VbsPath
if (-not (Test-Path $VbsDir)) { New-Item -ItemType Directory -Path $VbsDir -Force | Out-Null }
$VbsContent = @"
Dim WshShell
Set WshShell = CreateObject("WScript.Shell")
WshShell.Run chr(34) & "$CoreExe" & chr(34) & " -d " & chr(34) & "$DataDir" & chr(34), 0, False
"@
# VBScript 仅支持 ANSI/ASCII 编码，禁止写入 UTF-8 BOM，否则 wscript 报 800A0408 无效字符错误
[System.IO.File]::WriteAllText($VbsPath, $VbsContent, [System.Text.Encoding]::ASCII)

Write-Host "[2/3] 生成静默启动引导器 (无黑框):" -ForegroundColor Green
Write-Host "      $VbsPath"
Write-Host ""

# 4. 配置并注册 Windows 计划任务
Write-Host "[3/3] 注册 Windows 计划任务 (ClashTuiMihomo)..." -ForegroundColor Green

$Action = New-ScheduledTaskAction -Execute "wscript.exe" -Argument "`"$VbsPath`"" -WorkingDirectory $WorkDir
$Trigger = New-ScheduledTaskTrigger -AtLogOn -User $env:USERNAME
$Principal = New-ScheduledTaskPrincipal -UserId $env:USERNAME -LogonType Interactive -RunLevel Highest
$Settings = New-ScheduledTaskSettingsSet -AllowStartIfOnBatteries -DontStopIfGoingOnBatteries -ExecutionTimeLimit ([TimeSpan]::Zero)

try {
    Register-ScheduledTask -TaskName "ClashTuiMihomo" -Action $Action -Trigger $Trigger -Principal $Principal -Settings $Settings -Force | Out-Null
    
    # 验证是否成功注册
    $task = Get-ScheduledTask -TaskName "ClashTuiMihomo" -ErrorAction Stop
    
    Write-Host ""
    Write-Host "=========================================================" -ForegroundColor Green
    Write-Host " [成功] 开机自启服务已成功注册！" -ForegroundColor Green
    Write-Host "=========================================================" -ForegroundColor Green
    Write-Host " 1. 每次开机登录 Windows 后，Mihomo 内核将在后台静默自动拉起。"
    Write-Host " 2. 具备最高权限 (RunLevel Highest)，自动支持 TUN 虚拟网卡，开机免 UAC 弹窗。"
    Write-Host " 3. 支持笔记本电池模式与全天候不间断运行，无任何黑框或终端窗口干扰。"
    Write-Host " 4. 随时打开终端运行 clash-tui 即可管理节点，按 q 退出也不影响后台代理。"
    Write-Host " 5. 如需关闭自启，运行 unautostart.bat 或 unautostart.ps1 即可。"
    Write-Host "========================================================="
} catch {
    Write-Host ""
    Write-Host "[失败] 注册计划任务失败: $_" -ForegroundColor Red
}

Write-Host ""
Read-Host "按回车键退出"
