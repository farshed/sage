@echo off
setlocal
cd /d "%~dp0"

if not exist .env type nul > .env
cargo build --release --locked --workspace
if errorlevel 1 exit /b 1
target\release\sage.exe download-models
if errorlevel 1 exit /b 1

echo Setup complete. Configure your LLM in .env, then run target\release\sage.exe.
