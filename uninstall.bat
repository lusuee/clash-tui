@echo off
setlocal
chcp 65001 >nul

set "INSTALL_DIR=%LOCALAPPDATA%\clash-tui"

echo Removing %INSTALL_DIR% from user PATH ...
powershell -NoProfile -Command "$dir='%INSTALL_DIR%'; $p=[Environment]::GetEnvironmentVariable('Path','User'); $np=($p -split ';' | Where-Object { $_ -and ($_.TrimEnd('\') -ne $dir.TrimEnd('\')) }) -join ';'; [Environment]::SetEnvironmentVariable('Path',$np,'User')"

if not exist "%INSTALL_DIR%" (
    echo [DONE] Nothing installed at %INSTALL_DIR%.
    exit /b 0
)

echo.
echo WARNING: %INSTALL_DIR% contains kernel binary and your data
echo          (subscriptions / rules / profiles / logs).
choice /c YN /m "Delete the whole install directory"
if errorlevel 2 (
    echo [DONE] PATH entry removed. Data kept at %INSTALL_DIR%
    exit /b 0
)

rmdir /s /q "%INSTALL_DIR%"
echo [DONE] clash-tui uninstalled. Reopen terminal for PATH refresh.
