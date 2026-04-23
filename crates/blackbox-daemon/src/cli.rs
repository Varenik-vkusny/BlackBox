use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "blackbox")]
#[command(about = "BlackBox MCP server and dashboard")]
#[command(version = env!("CARGO_PKG_VERSION"))]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Run the daemon (default)
    Run {
        #[arg(long, default_value = "8765")]
        port: u16,
        #[arg(long)]
        cwd: Option<String>,
        #[arg(long)]
        capture_shell: bool,
        #[arg(long)]
        shell: Option<String>,
    },
    /// Configure MCP clients
    Setup {
        #[arg(long)]
        auto: bool,
    },
    /// Update to the latest release
    Update,
}
