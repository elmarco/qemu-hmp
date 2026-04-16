mod args;
mod commands;
mod completer;
mod expr;
mod format;
mod generated_registry;
mod highlighter;
mod prompt;
mod qmp;
mod repl;
mod terminal_image;

use std::io::IsTerminal;

use clap::Parser;

#[derive(Parser)]
#[command(name = "qemu-hmp", about = "External HMP monitor for QEMU")]
struct Cli {
    /// QMP socket path or address (unix:/path or tcp:host:port)
    #[arg(short = 's', long = "socket")]
    socket: String,

    /// Execute command(s) and exit (non-interactive batch mode)
    #[arg(short = 'c', long = "command")]
    commands: Vec<String>,

    /// Print completions for a partial input line and exit
    #[arg(long = "complete")]
    complete: Option<String>,

    /// Keep reconnecting on disconnect (retry every second)
    #[arg(short = 'k', long = "keep-alive")]
    keep_alive: bool,
}

async fn print_version(conn: &qmp::QmpConnection) {
    match conn.execute(qapi::qmp::query_version {}).await {
        Ok(info) => {
            let v = &info.qemu;
            eprintln!("QEMU {}.{}.{} connected.", v.major, v.minor, v.micro);
        }
        Err(_) => {
            eprintln!("Connected to QEMU.");
        }
    }
}

struct RawModeGuard;

impl RawModeGuard {
    fn new() -> Self {
        crossterm::terminal::enable_raw_mode().ok();
        Self
    }
}

impl Drop for RawModeGuard {
    fn drop(&mut self) {
        crossterm::terminal::disable_raw_mode().ok();
    }
}

/// Connect to QEMU with a spinner animation, retrying every second
/// until successful.  Enables raw mode during the spinner so that
/// keystrokes are not echoed, giving a "disabled input" feel.
async fn connect_with_retry(address: &str) -> qmp::QmpConnection {
    use std::io::Write;

    const FRAMES: &[&str] = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
    let mut frame = 0;

    let _raw = RawModeGuard::new();

    loop {
        match qmp::QmpConnection::connect(address).await {
            Ok(conn) => {
                eprint!("\r\x1b[2K");
                std::io::stderr().flush().ok();
                return conn;
            }
            Err(_) => {
                eprint!("\r\x1b[2K{} Connecting to {}...", FRAMES[frame], address);
                std::io::stderr().flush().ok();
                frame = (frame + 1) % FRAMES.len();
                tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            }
        }
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    if cli.keep_alive {
        let registry = commands::Registry::new();
        loop {
            let conn = connect_with_retry(&cli.socket).await;

            print_version(&conn).await;

            match repl::run(conn, &registry).await? {
                repl::ExitReason::UserQuit => return Ok(()),
                repl::ExitReason::Disconnected => continue,
            }
        }
    }

    let conn = match qmp::QmpConnection::connect(&cli.socket).await {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Failed to connect to QEMU at '{}': {}", cli.socket, e);
            eprintln!(
                "Make sure QEMU is running with: -qmp unix:{},server,wait=off",
                cli.socket
            );
            std::process::exit(1);
        }
    };

    print_version(&conn).await;

    let registry = commands::Registry::new();

    if let Some(ref line) = cli.complete {
        repl::run_complete(conn, &registry, line).await
    } else if !cli.commands.is_empty() {
        repl::run_batch(conn, &registry, &cli.commands).await
    } else if std::io::stdin().is_terminal() {
        repl::run(conn, &registry).await.map(|_| ())
    } else {
        repl::run_pipe(conn, &registry).await
    }
}
