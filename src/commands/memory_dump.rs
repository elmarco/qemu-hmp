// SPDX-License-Identifier: GPL-2.0-or-later

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::args::ArgValue;
use crate::commands::{require_expr, CmdError};
use crate::qmp::QmpConnection;

// x-debug-read-memory is not yet in the qapi-rs crate, define it locally.

#[derive(Debug, Deserialize)]
pub struct MemoryRead {
    pub data: String,
    #[serde(rename = "big-endian")]
    pub big_endian: bool,
    #[serde(rename = "addr-width")]
    pub addr_width: i64,
}

#[derive(Debug, Serialize)]
#[allow(non_camel_case_types)]
pub struct x_debug_read_memory {
    pub addr: i64,
    pub size: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub physical: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cpu: Option<i64>,
}

impl qapi_qmp::QmpCommand for x_debug_read_memory {}
impl qapi::Command for x_debug_read_memory {
    const NAME: &'static str = "x-debug-read-memory";
    const ALLOW_OOB: bool = false;
    type Ok = MemoryRead;
}

#[derive(Debug)]
struct FmtSpec {
    count: usize,
    format: char,
    wsize: usize,
}

fn parse_fmt(fmt: &str) -> Result<FmtSpec, CmdError> {
    let s = if let Some(stripped) = fmt.strip_prefix('/') {
        stripped
    } else {
        return Ok(FmtSpec {
            count: 1,
            format: 'x',
            wsize: 4,
        });
    };

    let mut chars = s.chars().peekable();

    // Parse count (digits)
    let mut count = 0usize;
    while let Some(&c) = chars.peek() {
        if c.is_ascii_digit() {
            count = count * 10 + (c as usize - '0' as usize);
            chars.next();
        } else {
            break;
        }
    }
    if count == 0 {
        count = 1;
    }

    // Parse format and size chars (in any order)
    let mut format: Option<char> = None;
    let mut wsize: Option<usize> = None;

    loop {
        match chars.peek() {
            Some(&c @ ('o' | 'd' | 'u' | 'x' | 'i' | 'c')) => {
                format = Some(c);
                chars.next();
            }
            Some(&'b') => {
                wsize = Some(1);
                chars.next();
            }
            Some(&'h') => {
                wsize = Some(2);
                chars.next();
            }
            Some(&'w') => {
                wsize = Some(4);
                chars.next();
            }
            Some(&('g' | 'L')) => {
                wsize = Some(8);
                chars.next();
            }
            _ => break,
        }
    }

    if let Some(&c) = chars.peek() {
        return Err(CmdError::Command(format!("invalid char in format: '{c}'")));
    }

    let format = format.unwrap_or('x');
    let wsize = if format == 'i' {
        // Disassembly mode: size is irrelevant
        0
    } else if format == 'c' {
        1
    } else {
        wsize.unwrap_or(4)
    };

    Ok(FmtSpec {
        count,
        format,
        wsize,
    })
}

fn read_value(data: &[u8], offset: usize, wsize: usize, big_endian: bool) -> u64 {
    match wsize {
        1 => data[offset] as u64,
        2 => {
            let bytes = [data[offset], data[offset + 1]];
            if big_endian {
                u16::from_be_bytes(bytes) as u64
            } else {
                u16::from_le_bytes(bytes) as u64
            }
        }
        4 => {
            let bytes = [
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
            ];
            if big_endian {
                u32::from_be_bytes(bytes) as u64
            } else {
                u32::from_le_bytes(bytes) as u64
            }
        }
        8 => {
            let bytes = [
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
                data[offset + 4],
                data[offset + 5],
                data[offset + 6],
                data[offset + 7],
            ];
            if big_endian {
                u64::from_be_bytes(bytes)
            } else {
                u64::from_le_bytes(bytes)
            }
        }
        _ => 0,
    }
}

pub(crate) fn format_char(v: u64) -> String {
    let c = (v & 0xff) as u8;
    match c {
        b'\'' => "'\\''".to_string(),
        b'\\' => "'\\\\'".to_string(),
        b'\n' => "'\\n'".to_string(),
        b'\r' => "'\\r'".to_string(),
        0x20..=0x7e => format!("'{}'", c as char),
        _ => format!("'\\x{:02x}'", c),
    }
}

pub async fn cmd_memory_dump(
    conn: &QmpConnection,
    args: &HashMap<String, ArgValue>,
    physical: bool,
) -> Result<String, CmdError> {
    let fmt_str = match args.get("fmt") {
        Some(ArgValue::Str(s)) => s.as_str(),
        _ => "",
    };
    let addr = require_expr(conn, args, "addr").await?;

    let spec = parse_fmt(fmt_str)?;

    if spec.format == 'i' {
        return Err(CmdError::Command(
            "disassembly format 'i' is not supported in the external HMP".to_string(),
        ));
    }

    let total_bytes = spec
        .count
        .checked_mul(spec.wsize)
        .ok_or_else(|| CmdError::Command("size overflow".to_string()))?;
    if total_bytes == 0 {
        return Ok(String::new());
    }

    let mem = conn
        .execute(x_debug_read_memory {
            addr,
            size: total_bytes as i64,
            physical: if physical { Some(true) } else { None },
            cpu: conn.cpu_index(),
        })
        .await
        .map_err(CmdError::from)?;

    // Decode hex data
    let data = hex::decode(&mem.data)
        .map_err(|e| CmdError::Command(format!("invalid hex data from QMP: {e}")))?;

    let addr_width = mem.addr_width as usize;
    let big_endian = mem.big_endian;

    // Compute max_digits for formatting
    let max_digits = match spec.format {
        'o' => (spec.wsize * 8).div_ceil(3),
        'x' => (spec.wsize * 8) / 4,
        'u' | 'd' => (spec.wsize * 8 * 10).div_ceil(33),
        'c' => 0,
        _ => (spec.wsize * 8) / 4,
    };

    let line_size = if spec.wsize == 1 { 8 } else { 16 };

    let mut output = String::new();
    let mut offset = 0usize;
    let mut cur_addr = addr as u64;
    let mut remaining = data.len();

    while remaining > 0 {
        let l = remaining.min(line_size);

        // Address
        output.push_str(&format!("{:0width$x}:", cur_addr, width = addr_width));

        // Values on this line
        let mut i = 0;
        while i < l {
            output.push(' ');
            let v = read_value(&data, offset + i, spec.wsize, big_endian);
            match spec.format {
                'o' => output.push_str(&format!("0{:width$o}", v, width = max_digits)),
                'x' => output.push_str(&format!("0x{:0width$x}", v, width = max_digits)),
                'u' => output.push_str(&format!("{:width$}", v, width = max_digits)),
                'd' => output.push_str(&format!("{:width$}", v as i64, width = max_digits)),
                'c' => output.push_str(&format_char(v)),
                _ => {}
            }
            i += spec.wsize;
        }
        output.push('\n');
        cur_addr += l as u64;
        offset += l;
        remaining -= l;
    }

    // Remove trailing newline to match QEMU's monitor_printf behavior
    // (the caller adds a final newline)
    if output.ends_with('\n') {
        output.pop();
    }

    Ok(output)
}

pub async fn cmd_x(
    conn: &QmpConnection,
    args: &HashMap<String, ArgValue>,
) -> Result<String, CmdError> {
    cmd_memory_dump(conn, args, false).await
}

pub async fn cmd_xp(
    conn: &QmpConnection,
    args: &HashMap<String, ArgValue>,
) -> Result<String, CmdError> {
    cmd_memory_dump(conn, args, true).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_fmt_default() {
        let spec = parse_fmt("").unwrap();
        assert_eq!(spec.count, 1);
        assert_eq!(spec.format, 'x');
        assert_eq!(spec.wsize, 4);
    }

    #[test]
    fn parse_fmt_count_format_size() {
        let spec = parse_fmt("/10xw").unwrap();
        assert_eq!(spec.count, 10);
        assert_eq!(spec.format, 'x');
        assert_eq!(spec.wsize, 4);
    }

    #[test]
    fn parse_fmt_just_format() {
        let spec = parse_fmt("/x").unwrap();
        assert_eq!(spec.count, 1);
        assert_eq!(spec.format, 'x');
        assert_eq!(spec.wsize, 4);
    }

    #[test]
    fn parse_fmt_count_only() {
        let spec = parse_fmt("/10").unwrap();
        assert_eq!(spec.count, 10);
        assert_eq!(spec.format, 'x');
        assert_eq!(spec.wsize, 4);
    }

    #[test]
    fn parse_fmt_size_format() {
        let spec = parse_fmt("/bx").unwrap();
        assert_eq!(spec.count, 1);
        assert_eq!(spec.format, 'x');
        assert_eq!(spec.wsize, 1);
    }

    #[test]
    fn parse_fmt_octal() {
        let spec = parse_fmt("/4og").unwrap();
        assert_eq!(spec.count, 4);
        assert_eq!(spec.format, 'o');
        assert_eq!(spec.wsize, 8);
    }

    #[test]
    fn parse_fmt_char() {
        let spec = parse_fmt("/16c").unwrap();
        assert_eq!(spec.count, 16);
        assert_eq!(spec.format, 'c');
        assert_eq!(spec.wsize, 1);
    }

    #[test]
    fn parse_fmt_invalid_char() {
        match parse_fmt("/10zw") {
            Err(CmdError::Command(msg)) => {
                assert!(msg.contains("invalid char"), "unexpected: {msg}");
            }
            other => panic!("expected error, got: {other:?}"),
        }
    }

    #[test]
    fn test_read_value_le() {
        let data = [0x78, 0x56, 0x34, 0x12];
        assert_eq!(read_value(&data, 0, 1, false), 0x78);
        assert_eq!(read_value(&data, 0, 2, false), 0x5678);
        assert_eq!(read_value(&data, 0, 4, false), 0x12345678);
    }

    #[test]
    fn test_read_value_be() {
        let data = [0x12, 0x34, 0x56, 0x78];
        assert_eq!(read_value(&data, 0, 1, true), 0x12);
        assert_eq!(read_value(&data, 0, 2, true), 0x1234);
        assert_eq!(read_value(&data, 0, 4, true), 0x12345678);
    }

    #[test]
    fn test_format_char() {
        assert_eq!(format_char(b'A' as u64), "'A'");
        assert_eq!(format_char(b'\n' as u64), "'\\n'");
        assert_eq!(format_char(b'\r' as u64), "'\\r'");
        assert_eq!(format_char(b'\'' as u64), "'\\''");
        assert_eq!(format_char(b'\\' as u64), "'\\\\'");
        assert_eq!(format_char(0x01), "'\\x01'");
        assert_eq!(format_char(0xff), "'\\xff'");
    }
}
