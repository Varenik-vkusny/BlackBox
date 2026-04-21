# Design: Start Daemon and Inject Test Data

This document outlines the process for initializing the BlackBox environment and populating the dashboard with sample data for testing purposes.

## Goal
Establish a running BlackBox daemon and feed it with a variety of log patterns (INFO, ERROR, PANIC) to verify the frontend's visualization capabilities.

## Architecture
- **Daemon**: Rust-based background process listening on port 8768.
- **Frontend**: Vite-based React application (already running).
- **Injector**: PowerShell script that sends POST requests to the daemon's `/api/inject` endpoint.

## Execution Steps
1. **Start Daemon**: Execute `.\start-daemon.ps1` from the project root.
   - This script builds the daemon if necessary and starts it in a hidden window.
2. **Inject Data**: Execute `powershell -ExecutionPolicy Bypass -File blackbox-lab\test-data-inject.ps1`.
   - This script sends a series of structured and unstructured logs to the daemon.

## Verification
- Confirm the daemon reports readiness on port 8768.
- Confirm the injector reports successful delivery of log items.
