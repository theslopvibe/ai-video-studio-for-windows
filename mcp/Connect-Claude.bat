@echo off
rem Double-click: connects Claude Desktop to AI Video Studio For Windows (no extension needed).
powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0Connect-Claude.ps1"
echo.
pause
