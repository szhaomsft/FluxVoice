@echo off
echo ========================================
echo FluxVoice Tauri Setup Guide
echo ========================================
echo.

echo Step 1: Check Node.js
node --version >nul 2>&1
if %errorlevel% neq 0 (
    echo [ERROR] Node.js not found!
    echo Please install Node.js from https://nodejs.org/
    pause
    exit /b 1
) else (
    echo [OK] Node.js is installed
    node --version
)
echo.

echo Step 2: Check Rust
rustc --version >nul 2>&1
if %errorlevel% neq 0 (
    echo [MISSING] Rust is not installed
    echo.
    echo Please install Rust:
    echo 1. Visit https://rustup.rs/
    echo 2. Download and run rustup-init.exe
    echo 3. Follow the installation prompts
    echo 4. Restart this terminal after installation
    echo.
    pause
    exit /b 1
) else (
    echo [OK] Rust is installed
    rustc --version
    cargo --version
)
echo.

echo Step 3: Install npm dependencies
echo Installing packages...
call npm install
if %errorlevel% neq 0 (
    echo [ERROR] npm install failed
    pause
    exit /b 1
) else (
    echo [OK] npm packages installed
)
echo.

echo ========================================
echo Setup Complete!
echo ========================================
echo.
echo To run FluxVoice in development mode:
echo   npm run tauri dev
echo.
echo To build for production:
echo   npm run tauri build
echo.
echo Configuration:
echo 1. Click the floating window to open settings
echo 2. Enter your Azure Speech Service credentials
echo 3. (Optional) Add Azure OpenAI credentials for text polishing
echo 4. Press Ctrl+F12 to start/stop recording
echo.
pause
