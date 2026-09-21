@echo off
setlocal
"%SystemRoot%\System32\WindowsPowerShell\v1.0\powershell.exe" -NoProfile -ExecutionPolicy Bypass -File "%~dp0Start-LittleBigMouse.ps1" -NoPause
if errorlevel 1 (
    echo.
    echo LittleBigMouse could not start. See the error above and launch-error.txt.
    pause
    exit /b 1
)
