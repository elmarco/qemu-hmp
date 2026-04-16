// SPDX-License-Identifier: GPL-2.0-or-later

use std::collections::HashMap;

use serde_json::Value;

use crate::args::ArgValue;
use crate::commands::{require_str, CmdError};
use crate::qmp::QmpConnection;

pub async fn cmd_qom_get(
    conn: &QmpConnection,
    args: &HashMap<String, ArgValue>,
) -> Result<String, CmdError> {
    let path = require_str(args, "path")?;
    let property = require_str(args, "property")?;
    let value = conn
        .execute(qapi::qmp::qom_get { path, property })
        .await
        .map_err(CmdError::from)?;
    Ok(json_pretty_4indent(&value))
}

/// Pretty-print a JSON value with 4-space indentation (matching QEMU).
fn json_pretty_4indent(value: &Value) -> String {
    let mut buf = String::new();
    json_fmt(value, 0, &mut buf);
    buf
}

fn json_fmt(value: &Value, depth: usize, buf: &mut String) {
    match value {
        Value::Null => buf.push_str("null"),
        Value::Bool(b) => buf.push_str(if *b { "true" } else { "false" }),
        Value::Number(n) => buf.push_str(&n.to_string()),
        Value::String(s) => {
            buf.push('"');
            for ch in s.chars() {
                match ch {
                    '"' => buf.push_str("\\\""),
                    '\\' => buf.push_str("\\\\"),
                    '\n' => buf.push_str("\\n"),
                    '\r' => buf.push_str("\\r"),
                    '\t' => buf.push_str("\\t"),
                    c if c < '\x20' => {
                        buf.push_str(&format!("\\u{:04x}", c as u32));
                    }
                    c => buf.push(c),
                }
            }
            buf.push('"');
        }
        Value::Array(arr) => {
            buf.push('[');
            for (i, item) in arr.iter().enumerate() {
                if i > 0 {
                    buf.push(',');
                }
                buf.push('\n');
                push_indent(buf, depth + 1);
                json_fmt(item, depth + 1, buf);
            }
            if !arr.is_empty() {
                buf.push('\n');
                push_indent(buf, depth);
            }
            buf.push(']');
        }
        Value::Object(map) => {
            buf.push('{');
            for (i, (key, val)) in map.iter().enumerate() {
                if i > 0 {
                    buf.push(',');
                }
                buf.push('\n');
                push_indent(buf, depth + 1);
                buf.push('"');
                buf.push_str(key);
                buf.push_str("\": ");
                json_fmt(val, depth + 1, buf);
            }
            if !map.is_empty() {
                buf.push('\n');
                push_indent(buf, depth);
            }
            buf.push('}');
        }
    }
}

fn push_indent(buf: &mut String, depth: usize) {
    for _ in 0..depth {
        buf.push_str("    ");
    }
}
