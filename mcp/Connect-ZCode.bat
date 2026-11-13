@echo off
rem Double-click: connects ZCode (GLM) to AI Video Studio For Windows (no extension needed).
powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0Connect-ZCode.ps1"
echo.
pause
