@echo off
setlocal enabledelayedexpansion
chcp 65001 >nul

set "INSTALL_DIR=%LOCALAPPDATA%\clash-tui"
set "CORE_EXE=%INSTALL_DIR%\bin\mihomo.exe"
set "DATA_DIR=%INSTALL_DIR%\data"
set "TASK_NAME=ClashTuiMihomo"

if "%~1"=="--help" goto show_help
if "%~1"=="/?" goto show_help
if "%~1"=="-h" goto show_help
if "%~1"=="--uninstall" goto do_uninstall
if "%~1"=="/uninstall" goto do_uninstall
if "%~1"=="-u" goto do_uninstall
if "%~1"=="--autostart-on" goto enable_autostart
if "%~1"=="--enable-autostart" goto enable_autostart
if "%~1"=="--autostart-off" goto disable_autostart
if "%~1"=="--disable-autostart" goto disable_autostart
if "%~1"=="--status" goto show_status
if "%~1"=="-a" goto install_autostart
if "%~1"=="--autostart" goto install_autostart
if "%~1"=="/autostart" goto install_autostart
if "%~1"=="--no-autostart" goto install_no_autostart
if "%~1"=="/no-autostart" goto install_no_autostart

goto install_interactive

:show_help
echo Clash TUI Windows 一体化安装与服务管理脚本
echo.
echo 用法: install.bat [选项]
echo.
echo 选项:
echo   (无参数)              交互式安装（安装 + 自动询问是否开启开机自启）
echo   --autostart, -a       安装并开启开机自启（静默无交互）
echo   --no-autostart        仅安装，不配置开机自启
echo   --uninstall, -u       卸载 clash-tui 并清理计划任务与 PATH
echo   --autostart-on        单独配置并启用开机自启计划任务
echo   --autostart-off       单独关闭并移除自启计划任务
echo   --status              查看安装与服务状态
echo   --help, -h, /?        显示此帮助信息
goto :eof

:enable_autostart
echo [配置自启] 正在配置 Windows 计划任务开机静默自启...
if not exist "%DATA_DIR%" mkdir "%DATA_DIR%"
set "TASK_RUN=\"%CORE_EXE%\" -d \"%DATA_DIR%\""
schtasks /create /tn "%TASK_NAME%" /tr "%TASK_RUN%" /sc onlogon /rl highest /f >nul 2>&1
if %errorlevel% equ 0 (
    echo ✔ [成功] 已注册 Windows 计划任务 (%TASK_NAME%)，登录时以最高权限静默启动。
) else (
    echo [提示] 注册最高权限计划任务需要管理员权限，正在请求提权...
    powershell -NoProfile -ExecutionPolicy Bypass -Command "Start-Process schtasks -ArgumentList '/create /tn %TASK_NAME% /tr \"%CORE_EXE% -d %DATA_DIR%\" /sc onlogon /rl highest /f' -Verb RunAs -Wait"
    echo ✔ [成功] 计划任务配置完成。
)
goto :eof

:disable_autostart
echo [取消自启] 正在移除开机自启计划任务...
schtasks /delete /tn "%TASK_NAME%" /f >nul 2>&1
if %errorlevel% equ 0 (
    echo ✔ [成功] 已取消 Windows 开机自启 (计划任务 %TASK_NAME% 已删除)。
) else (
    echo ℹ [提示] 未找到计划任务 %TASK_NAME% (可能尚未配置或已删除)。
)
goto :eof

:show_status
echo === Clash TUI 状态检查 ===
echo 安装路径: %INSTALL_DIR%
if exist "%INSTALL_DIR%\clash-tui.exe" (
    echo 主程序:   ✔ 已安装 (%INSTALL_DIR%\clash-tui.exe)
) else (
    echo 主程序:   ✘ 未安装
)
if exist "%CORE_EXE%" (
    echo 内核程序: ✔ 已安装 (%CORE_EXE%)
) else (
    echo 内核程序: ✘ 未找到
)
tasklist /fi "imagename eq mihomo.exe" 2>nul | findstr /i "mihomo.exe" >nul
if %errorlevel% equ 0 (
    echo 内核运行: ● 正在运行 (active)
) else (
    echo 内核运行: ○ 未运行 (inactive)
)
schtasks /query /tn "%TASK_NAME%" >nul 2>&1
if %errorlevel% equ 0 (
    echo 开机自启: ✔ 已配置计划任务 (%TASK_NAME%)
) else (
    echo 开机自启: ○ 未配置
)
echo =========================
goto :eof

:do_uninstall
echo [1/3] 移除开机自启计划任务...
call :disable_autostart

echo [2/3] 从用户 PATH 移除...
powershell -NoProfile -Command "$dir='%INSTALL_DIR%'; $p=[Environment]::GetEnvironmentVariable('Path','User'); if($p){ $newPath = (($p -split ';') | Where-Object { $_ -ne $dir }) -join ';'; [Environment]::SetEnvironmentVariable('Path', $newPath, 'User'); Write-Host '✔ 已从用户 PATH 移除' }"

echo [3/3] 询问数据清理...
if exist "%INSTALL_DIR%" (
    set /p "delData=是否删除用户配置与订阅数据 (%INSTALL_DIR%)？[y/N]: "
    if /i "!delData!"=="y" (
        rmdir /s /q "%INSTALL_DIR%"
        echo ✔ 已彻底删除安装目录与所有数据。
    ) else (
        echo ℹ 保留数据目录: %INSTALL_DIR%
    )
)
echo.
echo ✔ [完成] Clash TUI 已成功卸载。
goto :eof

:install_autostart
call :perform_install
call :enable_autostart
call :finish_install
goto :eof

:install_no_autostart
call :perform_install
echo ℹ 已跳过开机自启配置。
call :finish_install
goto :eof

:install_interactive
call :perform_install
echo.
echo ==========================================================
set /p "ans=是否配置开机后台静默自启（推荐：系统登录时自动运行代理）？[Y/n]: "
if "!ans!"=="" set "ans=y"
if /i "!ans!"=="y" (
    call :enable_autostart
) else (
    echo ℹ 已跳过自启配置。后续如需开启可随时运行: install.bat --autostart-on
)
call :finish_install
goto :eof

:perform_install
set "EXE=%~dp0clash-tui.exe"
if not exist "%EXE%" set "EXE=%~dp0target\release\clash-tui.exe"

if not exist "%EXE%" (
    echo [ERROR] 未找到 clash-tui.exe 或 target\release\clash-tui.exe
    echo         请先从 Release 下载预编译包或运行: cargo build --release
    exit /b 1
)

echo [1/3] 安装程序至 %INSTALL_DIR% ...
if not exist "%INSTALL_DIR%" mkdir "%INSTALL_DIR%"
copy /y "%EXE%" "%INSTALL_DIR%\clash-tui.exe" >nul

echo [2/3] 同步内核与初始配置 ...
if not exist "%INSTALL_DIR%\bin" mkdir "%INSTALL_DIR%\bin"
if not exist "%CORE_EXE%" (
    if exist "%~dp0bin\mihomo.exe" copy /y "%~dp0bin\mihomo.exe" "%INSTALL_DIR%\bin\" >nul
)
if not exist "%INSTALL_DIR%\subscriptions.json" if exist "%~dp0subscriptions.json" copy /y "%~dp0subscriptions.json" "%INSTALL_DIR%\" >nul
if not exist "%INSTALL_DIR%\rules.json" if exist "%~dp0rules.json" copy /y "%~dp0rules.json" "%INSTALL_DIR%\" >nul
if not exist "%INSTALL_DIR%\profiles" if exist "%~dp0profiles" xcopy /e /i /q "%~dp0profiles" "%INSTALL_DIR%\profiles" >nul
if not exist "%INSTALL_DIR%\data" if exist "%~dp0data" xcopy /e /i /q "%~dp0data" "%INSTALL_DIR%\data" >nul

echo [3/3] 添加至用户环境变量 PATH ...
powershell -NoProfile -Command "$dir='%INSTALL_DIR%'; $p=[Environment]::GetEnvironmentVariable('Path','User'); if(($p -split ';') -notcontains $dir){ [Environment]::SetEnvironmentVariable('Path', ($p.TrimEnd(';')+';'+$dir), 'User'); Write-Host '✔ [成功] 已加入用户 PATH' } else { Write-Host '✔ [已存在] 已在用户 PATH' }"
goto :eof

:finish_install
echo.
echo ==========================================================
echo ✔ [安装成功] Clash TUI 已就绪！
echo.
echo 新开一个终端，在任意目录输入以下命令即可运行:
echo     clash-tui
echo.
echo 服务管理命令速查:
echo   开启开机自启:  install.bat --autostart-on (或 clash-tui autostart on)
echo   关闭开机自启:  install.bat --autostart-off (或 clash-tui autostart off)
echo   查看当前状态:  install.bat --status (或 clash-tui autostart status)
echo   完整卸载软件:  install.bat --uninstall
echo ==========================================================
goto :eof
