// SPDX-License-Identifier: GPL-2.0-or-later

use chrono::{Local, TimeZone};
use nu_ansi_term::Color;

pub fn format_event(event: &serde_json::Value) -> String {
    let Some(obj) = event.as_object() else {
        return format!("{event}");
    };

    let name = obj
        .get("event")
        .and_then(|v| v.as_str())
        .unwrap_or("UNKNOWN");
    let data = obj.get("data").unwrap_or(&serde_json::Value::Null);
    let ts = obj.get("timestamp").unwrap_or(&serde_json::Value::Null);
    let secs = ts["seconds"].as_u64().unwrap_or(0);
    let us = ts["microseconds"].as_u64().unwrap_or(0);

    let data_str = format_event_data(data);
    let time_str = format_event_time(secs, us);

    if data_str.is_empty() {
        format!("✨ {name} [{time_str}]")
    } else {
        format!("✨ {name} {data_str} [{time_str}]")
    }
}

/// Format event data as `{ key: value, ... }` with Rust-style field names.
fn format_event_data(value: &serde_json::Value) -> String {
    let Some(map) = value.as_object() else {
        return String::new();
    };
    if map.is_empty() {
        return String::new();
    }

    let pairs: Vec<String> = map
        .iter()
        .map(|(k, v)| {
            let key = k.replace('-', "_");
            format_kv(&key, v)
        })
        .collect();
    format!("{{ {} }}", pairs.join(", "))
}

/// Format a single key-value pair, inlining nested objects.
fn format_kv(key: &str, v: &serde_json::Value) -> String {
    match v {
        serde_json::Value::String(s) => format!("{key}: \"{s}\""),
        serde_json::Value::Bool(b) => format!("{key}: {b}"),
        serde_json::Value::Number(n) => format!("{key}: {n}"),
        serde_json::Value::Null => format!("{key}: null"),
        serde_json::Value::Array(arr) => {
            let items: Vec<String> = arr.iter().map(|item| format!("{item}")).collect();
            format!("{key}: [{}]", items.join(", "))
        }
        serde_json::Value::Object(map) => {
            let inner: Vec<String> = map
                .iter()
                .map(|(ik, iv)| {
                    let ik = ik.replace('-', "_");
                    format_kv(&ik, iv)
                })
                .collect();
            format!("{key}: {{ {} }}", inner.join(", "))
        }
    }
}

/// Format a UNIX timestamp as `HH:MM:SS.mmm` in the local timezone.
fn format_event_time(seconds: u64, microseconds: u64) -> String {
    let secs = seconds as i64;
    let nanos = (microseconds * 1000) as u32;
    let dt = Local
        .timestamp_opt(secs, nanos)
        .single()
        .unwrap_or_else(|| Local.timestamp_opt(0, 0).single().unwrap());
    dt.format("%H:%M:%S%.3f").to_string()
}

/// Pretty-print a `serde_json::Value` as human-readable output.
///
/// Objects and arrays are formatted with indentation; scalars are printed
/// on a single line.  This is the "JSON fallback" used when no specialised
/// formatter exists for a command's response.
#[allow(dead_code)]
pub fn json_fallback(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::Null => String::new(),
        serde_json::Value::String(s) => s.clone(),
        other => serde_json::to_string_pretty(other).unwrap_or_else(|_| format!("{other}")),
    }
}

pub fn json_colored(value: &serde_json::Value) -> String {
    let mut buf = String::new();
    json_colored_recursive(value, &mut buf, 0);
    buf
}

fn json_colored_recursive(value: &serde_json::Value, buf: &mut String, indent: usize) {
    use std::fmt::Write;

    match value {
        serde_json::Value::Null => {
            write!(buf, "{}", Color::DarkGray.paint("null")).unwrap();
        }
        serde_json::Value::Bool(b) => {
            write!(buf, "{}", Color::Yellow.paint(b.to_string())).unwrap();
        }
        serde_json::Value::Number(n) => {
            write!(buf, "{}", Color::Cyan.paint(n.to_string())).unwrap();
        }
        serde_json::Value::String(s) => {
            write!(buf, "{}", Color::Green.paint(format!("\"{s}\""))).unwrap();
        }
        serde_json::Value::Array(arr) => {
            if arr.is_empty() {
                buf.push_str("[]");
                return;
            }
            buf.push_str("[\n");
            for (i, v) in arr.iter().enumerate() {
                write!(buf, "{:width$}", "", width = indent + 2).unwrap();
                json_colored_recursive(v, buf, indent + 2);
                if i + 1 < arr.len() {
                    buf.push(',');
                }
                buf.push('\n');
            }
            write!(buf, "{:width$}]", "", width = indent).unwrap();
        }
        serde_json::Value::Object(map) => {
            if map.is_empty() {
                buf.push_str("{}");
                return;
            }
            buf.push_str("{\n");
            for (i, (k, v)) in map.iter().enumerate() {
                write!(
                    buf,
                    "{:width$}{}: ",
                    "",
                    Color::Blue.bold().paint(format!("\"{k}\"")),
                    width = indent + 2
                )
                .unwrap();
                json_colored_recursive(v, buf, indent + 2);
                if i + 1 < map.len() {
                    buf.push(',');
                }
                buf.push('\n');
            }
            write!(buf, "{:width$}}}", "", width = indent).unwrap();
        }
    }
}

/// Format a byte count as a human-readable size string using binary
/// prefixes (KiB, MiB, GiB, TiB).
///
/// Values that are an exact multiple of a power of 1024 are shown without
/// a fractional part; otherwise two decimal places are used.  Values
/// below 1024 are shown in bytes.
///
/// Uses integer-only arithmetic to avoid precision loss when casting
/// large `i64` values to `f64` (which only has 53 bits of mantissa).
pub fn format_size(bytes: i64) -> String {
    const KIB: u64 = 1024;
    const MIB: u64 = KIB * 1024;
    const GIB: u64 = MIB * 1024;
    const TIB: u64 = GIB * 1024;

    let abs = bytes.unsigned_abs();
    let sign = if bytes < 0 { "-" } else { "" };

    let (divisor, unit) = if abs >= TIB {
        (TIB, "TiB")
    } else if abs >= GIB {
        (GIB, "GiB")
    } else if abs >= MIB {
        (MIB, "MiB")
    } else if abs >= KIB {
        (KIB, "KiB")
    } else {
        return format!("{bytes} bytes");
    };

    let whole = abs / divisor;
    let remainder = abs % divisor;

    if remainder == 0 {
        format!("{sign}{whole} {unit}")
    } else {
        // Two decimal places with rounding.
        let hundredths = (remainder * 100 + divisor / 2) / divisor;
        if hundredths >= 100 {
            format!("{sign}{} {unit}", whole + 1)
        } else {
            format!("{sign}{whole}.{hundredths:02} {unit}")
        }
    }
}

/// Convert reStructuredText from .hx SRST blocks to terminal text.
///
/// When `styled` is true, applies ANSI formatting: double backticks
/// become **bold**, single asterisks become _underlined_ (like
/// man-page conventions for commands vs. arguments).  When false,
/// the markup is simply stripped.
///
/// Backslash escapes are resolved and common leading indentation
/// is removed in both modes.
pub fn rst_to_text(rst: &str, styled: bool) -> String {
    let bold = if styled { "\x1b[1m" } else { "" };
    let underline = if styled { "\x1b[4m" } else { "" };
    let reset_bold = if styled { "\x1b[0m" } else { "" };
    let reset_ul = if styled { "\x1b[0m" } else { "" };

    // Step 1: Dedent (on raw text before ANSI codes are inserted).
    let min_indent = rst
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| l.len() - l.trim_start().len())
        .min()
        .unwrap_or(0);

    let dedented: String = rst
        .lines()
        .map(|l| {
            if l.len() >= min_indent {
                &l[min_indent..]
            } else {
                l.trim()
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
    let dedented = dedented.trim();

    // Step 2: Resolve backslash escapes (e.g. `\=`, `\ `).
    let mut cleaned = String::with_capacity(dedented.len());
    let mut chars = dedented.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\\' {
            if let Some(&next) = chars.peek() {
                if !next.is_alphanumeric() {
                    chars.next();
                    if next != ' ' {
                        cleaned.push(next);
                    }
                    continue;
                }
            }
        }
        cleaned.push(c);
    }

    // Step 3: Replace ``text`` with bold.
    let mut result = String::new();
    for (i, part) in cleaned.split("``").enumerate() {
        if i % 2 == 1 {
            result.push_str(bold);
            result.push_str(part);
            result.push_str(reset_bold);
        } else {
            result.push_str(part);
        }
    }

    // Step 4: Replace *text* with underline.
    style_emphasis(&result, underline, reset_ul)
}

/// Replace RST emphasis markers (`*text*`) with ANSI-styled text.
///
/// Skips `**` (strong emphasis) and toggles the given ANSI codes
/// around each `*`-delimited span.
fn style_emphasis(s: &str, open: &str, close: &str) -> String {
    let mut out = String::with_capacity(s.len() + 64);
    let chars: Vec<char> = s.chars().collect();
    let len = chars.len();
    let mut i = 0;
    let mut in_emphasis = false;

    while i < len {
        if chars[i] == '*' {
            if i + 1 < len && chars[i + 1] == '*' {
                out.push_str("**");
                i += 2;
                continue;
            }
            if in_emphasis {
                out.push_str(close);
                in_emphasis = false;
            } else {
                out.push_str(open);
                in_emphasis = true;
            }
            i += 1;
        } else {
            out.push(chars[i]);
            i += 1;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_fallback_null() {
        assert_eq!(json_fallback(&serde_json::Value::Null), "");
    }

    #[test]
    fn json_fallback_string() {
        let v = serde_json::Value::String("hello".into());
        assert_eq!(json_fallback(&v), "hello");
    }

    #[test]
    fn json_fallback_object() {
        let v: serde_json::Value = serde_json::json!({"a": 1});
        let out = json_fallback(&v);
        assert!(out.contains("\"a\""));
        assert!(out.contains('1'));
    }

    #[test]
    fn format_size_bytes() {
        assert_eq!(format_size(512), "512 bytes");
        assert_eq!(format_size(0), "0 bytes");
    }

    #[test]
    fn format_size_kib() {
        assert_eq!(format_size(1024), "1 KiB");
        assert_eq!(format_size(2048), "2 KiB");
    }

    #[test]
    fn format_size_gib() {
        assert_eq!(format_size(10 * 1024 * 1024 * 1024), "10 GiB");
    }

    #[test]
    fn format_size_fractional() {
        // 1.5 GiB
        let bytes = 1024 * 1024 * 1024 + 512 * 1024 * 1024;
        assert_eq!(format_size(bytes), "1.50 GiB");
    }

    #[test]
    fn format_size_tib() {
        assert_eq!(format_size(1024_i64 * 1024 * 1024 * 1024), "1 TiB");
    }

    #[test]
    fn format_size_large_value_no_precision_loss() {
        // 2^53 + 1024^3 bytes — above f64 mantissa precision.
        // With i64-as-f64 cast this would silently round, producing
        // wrong output.  Integer arithmetic handles it exactly.
        let val = (1_i64 << 53) + 1024 * 1024 * 1024;
        let out = format_size(val);
        assert_eq!(out, "8192.00 TiB");
    }

    #[test]
    fn event_time_formatting() {
        // Use chrono to compute the expected local-time string.
        let secs: u64 = 12 * 3600 + 30 * 60 + 45;
        let us: u64 = 123_456;
        let dt = Local
            .timestamp_opt(secs as i64, (us * 1000) as u32)
            .single()
            .unwrap();
        let expected = dt.format("%H:%M:%S%.3f").to_string();
        assert_eq!(format_event_time(secs, us), expected);
    }

    #[test]
    fn event_time_midnight_utc() {
        // Epoch 0 — local time depends on timezone.
        let dt = Local.timestamp_opt(0, 0).single().unwrap();
        let expected = dt.format("%H:%M:%S%.3f").to_string();
        assert_eq!(format_event_time(0, 0), expected);
    }

    #[test]
    fn event_data_empty_object() {
        let v = serde_json::json!({});
        assert_eq!(format_event_data(&v), "");
    }

    #[test]
    fn event_data_simple_fields() {
        let v = serde_json::json!({"device": "ide1-cd0", "tray-open": true});
        let out = format_event_data(&v);
        assert!(out.starts_with("{ "));
        assert!(out.ends_with(" }"));
        assert!(out.contains("device: \"ide1-cd0\""));
        // Hyphens in keys are replaced with underscores.
        assert!(out.contains("tray_open: true"));
    }

    #[test]
    fn event_format_with_data() {
        let secs: u64 = 45045;
        let us: u64 = 230000;
        let event = serde_json::json!({
            "event": "DEVICE_TRAY_MOVED",
            "data": {
                "device": "ide1-cd0",
                "id": "/machine/unattached/device[4]",
                "tray-open": true
            },
            "timestamp": { "seconds": secs, "microseconds": us }
        });
        let expected_time = format_event_time(secs, us);
        let out = format_event(&event);
        assert!(out.starts_with("✨ DEVICE_TRAY_MOVED {"), "got: {out}");
        assert!(out.contains("device: \"ide1-cd0\""), "got: {out}");
        assert!(out.contains("tray_open: true"), "got: {out}");
        assert!(out.ends_with(&format!("[{expected_time}]")), "got: {out}");
    }

    #[test]
    fn rst_to_text_plain() {
        let rst = "``commit``\n  Commit changes to the disk images.";
        assert_eq!(
            rst_to_text(rst, false),
            "commit\n  Commit changes to the disk images."
        );
    }

    #[test]
    fn rst_to_text_styled() {
        let rst = "``commit``\n  Commit changes to the disk images.";
        assert_eq!(
            rst_to_text(rst, true),
            "\x1b[1mcommit\x1b[0m\n  Commit changes to the disk images."
        );
    }

    #[test]
    fn rst_to_text_emphasis() {
        let rst = "``help`` or ``?`` [*cmd*]\n  Show the help for command *cmd*.";
        assert_eq!(
            rst_to_text(rst, true),
            "\x1b[1mhelp\x1b[0m or \x1b[1m?\x1b[0m [\x1b[4mcmd\x1b[0m]\n  Show the help for command \x1b[4mcmd\x1b[0m."
        );
    }

    #[test]
    fn rst_to_text_dedent_info() {
        // Info subcommands have 2 extra spaces of indent.
        let rst = "  ``info version``\n    Show the version of QEMU.";
        assert_eq!(
            rst_to_text(rst, false),
            "info version\n  Show the version of QEMU."
        );
    }

    #[test]
    fn rst_to_text_backslash_escape() {
        let rst = "``log`` *item1*\\ [,...]\n  Activate logging.";
        assert_eq!(
            rst_to_text(rst, false),
            "log item1[,...]\n  Activate logging."
        );
    }

    #[test]
    fn event_format_empty_data() {
        let event = serde_json::json!({
            "event": "STOP",
            "data": {},
            "timestamp": { "seconds": 0, "microseconds": 0 }
        });
        let expected_time = format_event_time(0, 0);
        let out = format_event(&event);
        assert_eq!(out, format!("✨ STOP [{expected_time}]"));
    }
}
