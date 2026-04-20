use std::io::Read;
use std::thread;
use portable_pty::{CommandBuilder, native_pty_system, PtySize};

use crate::buffer::{push_line_and_drain, SharedBuffer};
use crate::scanners::drain::SharedDrainState;
use crate::structured_store::SharedStructuredStore;

pub fn run_pty_capture(
    buf: SharedBuffer,
    drain: SharedDrainState,
    structured: SharedStructuredStore,
    command: Option<String>,
) {
    let pty_system = native_pty_system();
    
    // Default to powershell on windows, sh on others
    let shell = if cfg!(windows) {
        command.unwrap_or_else(|| "powershell.exe".to_string())
    } else {
        command.unwrap_or_else(|| "sh".to_string())
    };

    let pair = pty_system
        .openpty(PtySize {
            rows: 24,
            cols: 80,
            pixel_width: 0,
            pixel_height: 0,
        })
        .expect("Failed to open PTY");

    let cmd = CommandBuilder::new(shell);
    let _child = pair.slave.spawn_command(cmd).expect("Failed to spawn shell in PTY");

    let mut reader = pair.master.try_clone_reader().expect("Failed to clone PTY reader");
    
    // Background thread to read from PTY
    thread::spawn(move || {
        // Keep pair alive in this thread's scope
        let _pty = pair;
        let mut buffer = [0u8; 4096];
        let mut line_buf = Vec::new();

        while let Ok(n) = reader.read(&mut buffer) {
            if n == 0 {
                break;
            }

            for &byte in &buffer[..n] {
                if byte == b'\n' {
                    if let Ok(line) = String::from_utf8(line_buf.clone()) {
                        push_line_and_drain(
                            &buf,
                            &drain,
                            &structured,
                            line,
                            Some("native-pty".to_string()),
                        );
                    }
                    line_buf.clear();
                } else if byte != b'\r' {
                    line_buf.push(byte);
                }
            }

            // If we have a "dangling" prompt or partial line, push it now.
            // This ensures PS C:\> and other prompts are visible immediately.
            if !line_buf.is_empty() {
                if let Ok(line) = String::from_utf8(line_buf.clone()) {
                    push_line_and_drain(
                        &buf,
                        &drain,
                        &structured,
                        line,
                        Some("native-pty".to_string()),
                    );
                }
                // We clear it so we don't push it again. 
                // NOTE: This means a very long line split across multiple read() calls
                // will be treated as multiple log lines. This is acceptable for Phase 3.
                line_buf.clear();
            }
        }
        
        eprintln!("BlackBox: native-pty session ended");
    });

    // We don't wait for child here, as the daemon should keep running.
    // If the child exits, the reader will eventually return 0.
}
