// SPDX-License-Identifier: GPL-2.0-or-later

use std::sync::Arc;

use reedline::{
    default_emacs_keybindings, ColumnarMenu, EditCommand, Emacs, FileBackedHistory, KeyCode,
    KeyModifiers, Keybindings, MenuBuilder, MouseClickMode, Reedline, ReedlineEvent, ReedlineMenu,
    Signal,
};

use crate::commands::{DispatchOutput, Registry};
use crate::completer::HmpCompleter;
use crate::highlighter::HmpHighlighter;
use crate::prompt::HmpPrompt;
use crate::qmp::QmpConnection;

/// Add Tab key binding for the completion menu.
fn add_menu_keybindings(keybindings: &mut Keybindings) {
    keybindings.add_binding(
        KeyModifiers::NONE,
        KeyCode::Tab,
        ReedlineEvent::UntilFound(vec![
            ReedlineEvent::Menu("completion_menu".to_string()),
            ReedlineEvent::MenuNext,
        ]),
    );
    keybindings.add_binding(
        KeyModifiers::SHIFT,
        KeyCode::BackTab,
        ReedlineEvent::MenuPrevious,
    );
    keybindings.add_binding(
        KeyModifiers::ALT,
        KeyCode::Enter,
        ReedlineEvent::Edit(vec![EditCommand::InsertNewline]),
    );
}

/// Print completions for a partial input line and exit.
pub async fn run_complete(
    conn: QmpConnection,
    registry: &Registry,
    line: &str,
) -> anyhow::Result<()> {
    use reedline::Completer;

    let conn = Arc::new(conn);
    let mut completer = HmpCompleter::new(conn, registry);
    let pos = line.len();
    let suggestions = completer.complete(line, pos);
    for s in suggestions {
        println!("{}", s.value);
    }
    Ok(())
}

/// Run commands non-interactively and exit.
pub async fn run_batch(
    conn: QmpConnection,
    registry: &Registry,
    commands: &[String],
) -> anyhow::Result<()> {
    for cmd in commands {
        match registry.dispatch(&conn, cmd, false).await {
            DispatchOutput::Output(output) => {
                if !output.is_empty() {
                    println!("{output}");
                }
            }
            DispatchOutput::Disconnected => {
                eprintln!("QEMU disconnected.");
                std::process::exit(1);
            }
        }
    }
    Ok(())
}

/// Read commands from stdin (pipe mode) with NUL record separators.
///
/// When stdin is not a terminal, this mode reads one command per line,
/// prints the output followed by a NUL byte (`\0`) after each response.
/// This allows a parent process to keep a single qemu-hmp instance alive
/// with a persistent QMP connection, sending commands through stdin and
/// reading responses delimited by `\0`.
pub async fn run_pipe(conn: QmpConnection, registry: &Registry) -> anyhow::Result<()> {
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

    let stdin = BufReader::new(tokio::io::stdin());
    let mut stdout = tokio::io::stdout();
    let mut lines = stdin.lines();

    while let Some(line) = lines.next_line().await? {
        let line = line.trim().to_string();
        if line.is_empty() {
            // Still emit the separator so the reader doesn't hang.
            stdout.write_all(b"\0").await?;
            stdout.flush().await?;
            continue;
        }
        match registry.dispatch(&conn, &line, false).await {
            DispatchOutput::Output(output) => {
                if !output.is_empty() {
                    stdout.write_all(output.as_bytes()).await?;
                    stdout.write_all(b"\n").await?;
                }
            }
            DispatchOutput::Disconnected => {
                eprintln!("QEMU disconnected.");
                std::process::exit(1);
            }
        }
        stdout.write_all(b"\0").await?;
        stdout.flush().await?;
    }
    Ok(())
}

/// Reason the REPL loop exited.
pub enum ExitReason {
    /// User pressed Ctrl-C or Ctrl-D.
    UserQuit,
    /// The QMP connection was closed by QEMU.
    Disconnected,
}

/// Run the interactive REPL loop.
pub async fn run(conn: QmpConnection, registry: &Registry) -> anyhow::Result<ExitReason> {
    let conn = Arc::new(conn);

    {
        // Notify the user immediately when QEMU closes the connection,
        // even if blocked in read_line.  The REPL loop will detect the
        // disconnect on the next iteration after read_line returns.
        let conn_watch = conn.clone();
        tokio::spawn(async move {
            use std::io::Write;

            conn_watch.wait_disconnected().await;
            // Clear the prompt line.  The terminal is in raw mode
            // (reedline), so use \r\n for newlines.
            eprint!("\r\x1b[2K");
            while let Some(event) = conn_watch.try_recv_event() {
                eprint!("{}\r\n", crate::format::format_event(&event));
            }
            eprint!("QEMU disconnected.\r\n");
            std::io::stderr().flush().ok();
        });
    }

    let completer = Box::new(HmpCompleter::new(conn.clone(), &registry));

    let columnar_menu = ColumnarMenu::default().with_name("completion_menu");
    let completion_menu = Box::new(columnar_menu);

    let mut keybindings = default_emacs_keybindings();
    add_menu_keybindings(&mut keybindings);
    let edit_mode = Box::new(Emacs::new(keybindings));

    let history_path = dirs::cache_dir()
        .unwrap_or_else(|| std::path::PathBuf::from(".cache"))
        .join("qemu-hmp")
        .join("history");
    let history = Box::new(FileBackedHistory::with_file(1000, history_path)?);

    let mut editor = Reedline::create()
        .with_completer(completer)
        .with_highlighter(Box::new(HmpHighlighter))
        .with_menu(ReedlineMenu::EngineCompleter(completion_menu))
        .with_edit_mode(edit_mode)
        .with_history(history)
        .with_mouse_click(MouseClickMode::Enabled);

    let prompt = HmpPrompt;
    let mut reason = ExitReason::UserQuit;

    loop {
        // Drain any pending events before showing the prompt.
        while let Some(event) = conn.try_recv_event() {
            eprintln!("{}", crate::format::format_event(&event));
        }

        if conn.is_disconnected() {
            eprintln!("QEMU disconnected.");
            reason = ExitReason::Disconnected;
            break;
        }

        match editor.read_line(&prompt) {
            Ok(Signal::Success(line)) => {
                let line = line.trim().to_string();
                if line.is_empty() {
                    continue;
                }
                match registry.dispatch(&conn, &line, true).await {
                    DispatchOutput::Output(output) => {
                        if !output.is_empty() {
                            println!("{output}");
                        }
                    }
                    DispatchOutput::Disconnected => {
                        eprintln!("QEMU disconnected.");
                        reason = ExitReason::Disconnected;
                        break;
                    }
                }
            }
            Ok(Signal::CtrlC) | Ok(Signal::CtrlD) => {
                break;
            }
            Ok(_) => {
                continue;
            }
            Err(e) => {
                eprintln!("Input error: {e}");
                break;
            }
        }
    }

    Ok(reason)
}
