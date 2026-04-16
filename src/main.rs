mod args;
mod commands;
mod completer;
mod discover;
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
    /// QMP socket path or address (unix:/path or tcp:host:port).
    /// If omitted, auto-discovers running QEMU QMP sockets.
    #[arg(short = 's', long = "socket")]
    socket: Option<String>,

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

fn resolve_socket(socket: Option<String>) -> anyhow::Result<String> {
    if let Some(s) = socket {
        return Ok(s);
    }

    let sockets = discover::find_qmp_sockets();
    match sockets.len() {
        0 => {
            eprintln!("No QEMU QMP sockets found.");
            eprintln!("Start QEMU with: -qmp unix:/tmp/qmp.sock,server,wait=off");
            std::process::exit(1);
        }
        1 => {
            eprintln!(
                "Auto-connecting to {} ({}).",
                sockets[0].label, sockets[0].path
            );
            Ok(sockets[0].path.clone())
        }
        _ => {
            if !std::io::stderr().is_terminal() {
                eprintln!("Multiple QMP sockets found; use -s to specify:");
                for s in &sockets {
                    eprintln!("  -s {}", s.path);
                }
                std::process::exit(1);
            }
            let idx = discover::select_socket(&sockets).map_err(|e| anyhow::anyhow!("{}", e))?;
            Ok(sockets[idx].path.clone())
        }
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    let socket = resolve_socket(cli.socket)?;

    if cli.keep_alive {
        let registry = commands::Registry::new();
        loop {
            let conn = connect_with_retry(&socket).await;

            print_version(&conn).await;

            match repl::run(conn, &registry).await? {
                repl::ExitReason::UserQuit => return Ok(()),
                repl::ExitReason::Disconnected => continue,
            }
        }
    }

    let conn = match qmp::QmpConnection::connect(&socket).await {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Failed to connect to QEMU at '{}': {}", socket, e);
            eprintln!(
                "Make sure QEMU is running with: -qmp unix:{},server,wait=off",
                socket
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
