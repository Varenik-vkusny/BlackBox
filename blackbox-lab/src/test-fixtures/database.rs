use std::net::TcpStream;
use std::time::Duration;

pub struct Database {
    connection_timeout: Duration,
}

impl Database {
    pub fn new(timeout_ms: u64) -> Self {
        Database {
            connection_timeout: Duration::from_millis(timeout_ms),
        }
    }

    pub fn connect(&self, host: &str, port: u16) -> Result<TcpStream, String> {
        // Line 18: This is where the panic occurs when connection is refused
        let addr = format!("{}:{}", host, port);
        match TcpStream::connect_timeout(&addr.parse().unwrap(), self.connection_timeout) {
            Ok(stream) => {
                stream.set_read_timeout(Some(self.connection_timeout)).ok();
                stream.set_write_timeout(Some(self.connection_timeout)).ok();
                Ok(stream)
            }
            Err(e) => {
                eprintln!("Failed to connect to {}: {}", addr, e);
                panic!("Database connection failed: {}", e); // Line 28
            }
        }
    }

    pub fn query(&self, sql: &str) -> Result<Vec<String>, String> {
        if sql.is_empty() {
            Err("Query cannot be empty".to_string())
        } else {
            Ok(vec![])
        }
    }
}
