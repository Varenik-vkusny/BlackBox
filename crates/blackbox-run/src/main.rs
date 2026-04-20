/// blackbox-run: wraps a process and tees its stdout/stderr into BlackBox's buffer.
///
/// Usage: blackbox-run <command> [args...]
///
/// The child process output is forwarded to the parent's stdout/stderr normally,
/// AND each line is sent to the BlackBox TCP bridge (port 8765) tagged with
/// the child's PID: {"t":"process:<pid>","d":"<line>"}.
///
/// If the TCP bridge is unavailable, the command still runs normally (no capture).

use std::io::{BufRead, BufReader, Write};
use std::net::TcpStream;
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;

const BRIDGE_ADDR: &str = "127.0.0.1:8765";

fn main() {
    let mut args = std::env::args().skip(1);
    let program = match args.next() {
        Some(p) => p,
        None => {
            eprintln!("Usage: blackbox-run <command> [args...]");
            std::process::exit(1);
        }
    };
    let rest: Vec<String> = args.collect();

    let mut child = match Command::new(&program)
        .args(&rest)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(c) => c,
        Err(e) => {
            eprintln!("blackbox-run: failed to start '{program}': {e}");
            std::process::exit(1);
        }
    };

    let pid = child.id();
    let tag = format!("process:{pid}");
    eprintln!("blackbox-run: capturing PID {pid} → BlackBox terminal '{tag}'");

    // Try to connect to the BlackBox TCP bridge; silently skip if unavailable.
    let tcp = TcpStream::connect(BRIDGE_ADDR).ok().map(|s| Arc::new(Mutex::new(s)));
    if tcp.is_none() {
        eprintln!("blackbox-run: BlackBox bridge not reachable at {BRIDGE_ADDR} — output passthrough only");
    }

    let child_stdout = child.stdout.take().expect("stdout pipe");
    let child_stderr = child.stderr.take().expect("stderr pipe");

    let tag_out = tag.clone();
    let tcp_out = tcp.clone();
    let stdout_thread = thread::spawn(move || {
        let reader = BufReader::new(child_stdout);
        let mut out = std::io::stdout();
        for line in reader.lines().flatten() {
            let _ = writeln!(out, "{line}");
            send_to_bridge(&tcp_out, &tag_out, &line);
        }
    });

    let tag_err = tag.clone();
    let tcp_err = tcp.clone();
    let stderr_thread = thread::spawn(move || {
        let reader = BufReader::new(child_stderr);
        let mut err = std::io::stderr();
        for line in reader.lines().flatten() {
            let _ = writeln!(err, "{line}");
            send_to_bridge(&tcp_err, &tag_err, &line);
        }
    });

    let status = child.wait().unwrap_or_else(|e| {
        eprintln!("blackbox-run: wait error: {e}");
        std::process::exit(1);
    });

    let _ = stdout_thread.join();
    let _ = stderr_thread.join();

    std::process::exit(status.code().unwrap_or(1));
}

fn send_to_bridge(tcp: &Option<Arc<Mutex<TcpStream>>>, tag: &str, line: &str) {
    if let Some(stream) = tcp {
        if let Ok(mut guard) = stream.lock() {
            let envelope = serde_json::json!({"t": tag, "d": line});
            let mut bytes = serde_json::to_vec(&envelope).unwrap_or_default();
            bytes.push(b'\n');
            // Ignore write errors — pipe may have closed
            let _ = guard.write_all(&bytes);
        }
    }
}
