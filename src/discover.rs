// SPDX-License-Identifier: GPL-2.0-or-later

use std::fs;
use std::io::{self, Write};
use std::path::Path;

use crossterm::event::{self, Event, KeyCode, KeyModifiers};

pub struct QmpSocket {
    pub path: String,
    pub label: String,
}

pub fn find_qmp_sockets() -> Vec<QmpSocket> {
    let proc_dir = match fs::read_dir("/proc") {
        Ok(d) => d,
        Err(_) => return Vec::new(),
    };

    let mut sockets = Vec::new();

    for entry in proc_dir.flatten() {
        let pid: u32 = match entry.file_name().to_str().and_then(|s| s.parse().ok()) {
            Some(p) => p,
            None => continue,
        };

        let exe = match fs::read_link(entry.path().join("exe")) {
            Ok(e) => e,
            Err(_) => continue,
        };

        let exe_name = exe.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if !exe_name.starts_with("qemu-system") && exe_name != "qemu-kvm" {
            continue;
        }

        let raw = match fs::read(entry.path().join("cmdline")) {
            Ok(c) => c,
            Err(_) => continue,
        };
        let args: Vec<&str> = raw
            .split(|&b| b == 0)
            .filter_map(|s| std::str::from_utf8(s).ok())
            .filter(|s| !s.is_empty())
            .collect();

        let mut vm_name: Option<String> = None;
        let mut qmp_paths: Vec<String> = Vec::new();

        let mut i = 0;
        while i < args.len() {
            match args[i] {
                "-qmp" | "-qmp-pretty" => {
                    if let Some(path) = args.get(i + 1).and_then(|s| extract_unix_path(s)) {
                        if Path::new(path).exists() {
                            qmp_paths.push(path.to_string());
                        }
                    }
                    i += 2;
                }
                "-name" => {
                    if let Some(val) = args.get(i + 1) {
                        let n = val.split(',').next().unwrap_or(val);
                        vm_name = Some(n.strip_prefix("guest=").unwrap_or(n).to_string());
                    }
                    i += 2;
                }
                _ => i += 1,
            }
        }

        for path in qmp_paths {
            let label = match &vm_name {
                Some(n) => format!("{} (pid {})", n, pid),
                None => format!("qemu (pid {})", pid),
            };
            sockets.push(QmpSocket { path, label });
        }
    }

    sockets.sort_by(|a, b| a.label.cmp(&b.label));
    sockets
}

fn extract_unix_path(spec: &str) -> Option<&str> {
    let rest = spec.strip_prefix("unix:")?;
    let path = rest.split(',').next()?;
    if path.is_empty() {
        None
    } else {
        Some(path)
    }
}

pub fn select_socket(sockets: &[QmpSocket]) -> io::Result<usize> {
    let mut selected = 0usize;
    let mut stderr = io::stderr();
    let total = sockets.len();

    crossterm::terminal::enable_raw_mode()?;
    let result = select_loop(&mut stderr, sockets, &mut selected, total);
    crossterm::terminal::disable_raw_mode().ok();

    for _ in 0..total + 1 {
        write!(stderr, "\x1b[A\x1b[2K")?;
    }
    stderr.flush()?;

    result
}

fn select_loop(
    w: &mut impl Write,
    sockets: &[QmpSocket],
    selected: &mut usize,
    total: usize,
) -> io::Result<usize> {
    draw_menu(w, sockets, *selected)?;

    loop {
        if let Event::Key(key) = event::read()? {
            match key.code {
                KeyCode::Up => {
                    *selected = selected.saturating_sub(1);
                }
                KeyCode::Down => {
                    if *selected + 1 < total {
                        *selected += 1;
                    }
                }
                KeyCode::Enter => {
                    return Ok(*selected);
                }
                KeyCode::Esc => {
                    return Err(io::Error::new(io::ErrorKind::Interrupted, "cancelled"));
                }
                KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    return Err(io::Error::new(io::ErrorKind::Interrupted, "cancelled"));
                }
                _ => continue,
            }

            for _ in 0..total + 1 {
                write!(w, "\x1b[A\r")?;
            }
            draw_menu(w, sockets, *selected)?;
        }
    }
}

fn draw_menu(w: &mut impl Write, sockets: &[QmpSocket], selected: usize) -> io::Result<()> {
    write!(w, "Select a QMP socket:\r\n")?;
    for (i, sock) in sockets.iter().enumerate() {
        if i == selected {
            write!(
                w,
                "\x1b[1;36m> {} \x1b[0;36m{}\x1b[0m\x1b[K\r\n",
                sock.label, sock.path
            )?;
        } else {
            write!(w, "  {} {}\x1b[K\r\n", sock.label, sock.path)?;
        }
    }
    w.flush()
}
