@echo off
setlocal
chcp 65001 >nul

net session >nul 2>&1
if %errorlevel% neq 0 (
    echo [提示] 正在请求管理员权限以移除开机自启任务...
    powershell -Command "Start-Process '%~f0' -Verb RunAs"
    exit /b
)

echo 正在注销开机自启任务 (ClashTuiMihomo)...
schtasks /delete /tn "ClashTuiMihomo" /f >nul 2>&1

if %errorlevel% equ 0 (
    echo.
    echo [成功] 已关闭开机自启任务！
) else (
    echo.
    echo [提示] 未找到名为 ClashTuiMihomo 的计划任务，或已提前删除。
)

echo.
pause
