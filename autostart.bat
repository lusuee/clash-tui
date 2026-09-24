@echo off
setlocal
chcp 65001 >nul

:: 检查管理员权限，若无则请求 UAC 提权以支持 TUN 模式免弹窗自启
net session >nul 2>&1
if %errorlevel% neq 0 (
    echo [提示] 正在请求管理员权限以配置开机自启任务...
    powershell -Command "Start-Process '%~f0' -Verb RunAs"
    exit /b
)

:: 探测内核与数据目录（优先项目目录，其次全局安装目录）
set "BASE_DIR=%~dp0"
if exist "%BASE_DIR%bin\mihomo.exe" (
    set "CORE_EXE=%BASE_DIR%bin\mihomo.exe"
    set "DATA_DIR=%BASE_DIR%data"
) else if exist "%LOCALAPPDATA%\clash-tui\bin\mihomo.exe" (
    set "CORE_EXE=%LOCALAPPDATA%\clash-tui\bin\mihomo.exe"
    set "DATA_DIR=%LOCALAPPDATA%\clash-tui\data"
) else (
    echo [错误] 未找到 mihomo.exe 内核文件！
    echo 请确认 bin\mihomo.exe 存在或已运行 install.bat 安装。
    pause
    exit /b 1
)

echo 正在注册 Windows 计划任务 (开机静默启动内核)...
echo 内核路径: "%CORE_EXE%"
echo 数据目录: "%DATA_DIR%"

schtasks /create /tn "ClashTuiMihomo" /tr "\"%CORE_EXE%\" -d \"%DATA_DIR%\"" /sc onlogon /rl highest /f >nul 2>&1

if %errorlevel% equ 0 (
    echo.
    echo =========================================================
    echo  [成功] 开机自启已配置完毕！
    echo =========================================================
    echo  - 每次开机登录 Windows 后，Mihomo 内核将在后台静默自启。
    echo  - 具备最高权限 (支持 TUN 虚拟网卡无感接管)。
    echo  - 无黑框控制台干扰，代理开机即可正常使用。
    echo  - 如需切换节点、测速或配置，直接运行 clash-tui 即可。
    echo  - 如需取消自启，随时双击运行 unautostart.bat。
    echo =========================================================
) else (
    echo.
    echo [失败] 计划任务创建失败，错误代码: %errorlevel%
)

echo.
pause
