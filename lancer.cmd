@echo off
setlocal
cd /d "%~dp0"
set "PATH=%LOCALAPPDATA%\Mvania\tools\mingw64\bin;%USERPROFILE%\.cargo\bin;%PATH%"
cargo run --release -- %*
if errorlevel 1 pause