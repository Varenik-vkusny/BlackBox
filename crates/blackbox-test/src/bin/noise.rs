use std::io::Write;
use std::net::TcpStream;
use std::thread;
use std::time::Duration;

const BRIDGE_PORT: u16 = 8765;

fn main() {
    println!("BlackBox Noise Generator started on port {BRIDGE_PORT}");
    println!("Sending random crashes to the daemon... Press Ctrl+C to stop.");

    let samples = vec![
        "thread 'main' panicked at 'index out of bounds', src/main.rs:42:15",
        "error[E0308]: mismatched types",
        "  --> src/api.rs:112:34",
        "   |",
        "112 |     let x: String = 42;",
        "   |                     ^^ expected `String`, found `integer`",
        "level=error msg=\"Connection refused\" container_id=\"abc123456789\"",
        "TypeError: Cannot read properties of undefined (reading 'map') at App.tsx:55",
        "FATAL: Out of memory in Node.js heap",
        "INFO: Processing request...",
        "DEBUG: Cache miss for key 'user_123'",
        "WARN: Low disk space on /var/lib/docker",
    ];

    loop {
        if let Ok(mut stream) = TcpStream::connect(format!("127.0.0.1:{BRIDGE_PORT}")) {
            let index = rand_index(samples.len());
            let line = samples[index];
            let _ = writeln!(stream, "{}", line);
            println!("Sent: {}", line);
        } else {
            eprintln!("Failed to connect to bridge on {BRIDGE_PORT}. Is the daemon running?");
        }
        thread::sleep(Duration::from_millis(2000));
    }
}

fn rand_index(max: usize) -> usize {
    use std::time::SystemTime;
    let n = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap().as_nanos();
    (n % max as u128) as usize
}
