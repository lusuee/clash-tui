@echo off
setlocal
chcp 65001 >nul

set "INSTALL_DIR=%LOCALAPPDATA%\clash-tui"
set "EXE=%~dp0target\release\clash-tui.exe"

if not exist "%EXE%" (
    echo [ERROR] Not found: %EXE%
    echo         Run "cargo build --release" first.
    exit /b 1
)

echo [1/3] Installing binary to %INSTALL_DIR% ...
if not exist "%INSTALL_DIR%" mkdir "%INSTALL_DIR%"
copy /y "%EXE%" "%INSTALL_DIR%\clash-tui.exe" >nul

echo [2/3] Seeding kernel and data on first install ...
if not exist "%INSTALL_DIR%\bin\mihomo.exe" if exist "%~dp0bin\mihomo.exe" (
    if not exist "%INSTALL_DIR%\bin" mkdir "%INSTALL_DIR%\bin"
    copy /y "%~dp0bin\mihomo.exe" "%INSTALL_DIR%\bin\" >nul
)
if not exist "%INSTALL_DIR%\subscriptions.json" if exist "%~dp0subscriptions.json" copy /y "%~dp0subscriptions.json" "%INSTALL_DIR%\" >nul
if not exist "%INSTALL_DIR%\rules.json" if exist "%~dp0rules.json" copy /y "%~dp0rules.json" "%INSTALL_DIR%\" >nul
if not exist "%INSTALL_DIR%\profiles" if exist "%~dp0profiles" xcopy /e /i /q "%~dp0profiles" "%INSTALL_DIR%\profiles" >nul
if not exist "%INSTALL_DIR%\data" if exist "%~dp0data" xcopy /e /i /q "%~dp0data" "%INSTALL_DIR%\data" >nul

echo [3/3] Adding to user PATH ...
powershell -NoProfile -Command "$dir='%INSTALL_DIR%'; $p=[Environment]::GetEnvironmentVariable('Path','User'); if(($p -split ';') -notcontains $dir){ [Environment]::SetEnvironmentVariable('Path', ($p.TrimEnd(';')+';'+$dir), 'User'); Write-Host '[OK] Added to user PATH' } else { Write-Host '[SKIP] Already in user PATH' }"

echo.
echo [DONE] Installed. Open a NEW terminal, then run anywhere:
echo     clash-tui
echo.
echo Data / kernel location: %INSTALL_DIR%
echo Uninstall: run uninstall.bat in the project root.
