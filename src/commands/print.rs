// SPDX-License-Identifier: GPL-2.0-or-later

use std::collections::HashMap;

use crate::args::ArgValue;
use crate::commands::memory_dump::format_char;
use crate::commands::{require_expr, CmdError};
use crate::qmp::QmpConnection;

/// Extract the format character from the `/fmt` argument.
/// Defaults to `'x'` when no format is specified (matching QEMU's default).
fn get_format(args: &HashMap<String, ArgValue>) -> char {
    if let Some(ArgValue::Str(s)) = args.get("fmt") {
        let s = s.strip_prefix('/').unwrap_or(s);
        for c in s.chars() {
            if matches!(c, 'o' | 'x' | 'u' | 'd' | 'c') {
                return c;
            }
        }
    }
    'x'
}

pub async fn cmd_print(
    conn: &QmpConnection,
    args: &HashMap<String, ArgValue>,
) -> Result<String, CmdError> {
    let val = require_expr(conn, args, "val").await?;

    let format = get_format(args);

    let formatted = match format {
        'o' => {
            // C %#PRIo64: "0" for zero, "0377" for 255 (0-prefix, not 0o)
            if val == 0 {
                "0".to_string()
            } else {
                format!("0{:o}", val as u64)
            }
        }
        'x' => {
            // C %#PRIx64: "0" for zero, "0xff" for 255
            if val == 0 {
                "0".to_string()
            } else {
                format!("{:#x}", val as u64)
            }
        }
        'u' => {
            format!("{}", val as u64)
        }
        'd' => {
            format!("{}", val)
        }
        'c' => format_char(val as u64),
        _ => format!("{}", val),
    };

    Ok(formatted)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_args(fmt: Option<&str>, val: &str) -> HashMap<String, ArgValue> {
        let mut args = HashMap::new();
        if let Some(f) = fmt {
            args.insert("fmt".to_string(), ArgValue::Str(f.to_string()));
        }
        args.insert("val".to_string(), ArgValue::Str(val.to_string()));
        args
    }

    #[test]
    fn test_get_format() {
        assert_eq!(get_format(&make_args(Some("/x"), "0")), 'x');
        assert_eq!(get_format(&make_args(Some("/o"), "0")), 'o');
        assert_eq!(get_format(&make_args(Some("/d"), "0")), 'd');
        assert_eq!(get_format(&make_args(Some("/u"), "0")), 'u');
        assert_eq!(get_format(&make_args(Some("/c"), "0")), 'c');
        assert_eq!(get_format(&make_args(None, "0")), 'x');
    }

    #[test]
    fn test_hex_format() {
        // C %#x: 0 → "0", 255 → "0xff"
        let fmt_hex = |v: i64| {
            if v == 0 {
                "0".to_string()
            } else {
                format!("{:#x}", v as u64)
            }
        };
        assert_eq!(fmt_hex(0), "0");
        assert_eq!(fmt_hex(255), "0xff");
        assert_eq!(fmt_hex(256), "0x100");
        assert_eq!(fmt_hex(-1), "0xffffffffffffffff");
    }

    #[test]
    fn test_octal_format() {
        // C %#o: 0 → "0", 255 → "0377"
        let fmt_oct = |v: i64| {
            if v == 0 {
                "0".to_string()
            } else {
                format!("0{:o}", v as u64)
            }
        };
        assert_eq!(fmt_oct(0), "0");
        assert_eq!(fmt_oct(255), "0377");
        assert_eq!(fmt_oct(8), "010");
    }

    #[test]
    fn test_decimal_formats() {
        assert_eq!(format!("{}", 255i64), "255");
        assert_eq!(format!("{}", -1i64), "-1");
        assert_eq!(format!("{}", 255u64), "255");
        assert_eq!(format!("{}", (-1i64) as u64), "18446744073709551615");
    }

    #[test]
    fn test_char_format() {
        assert_eq!(format_char(65), "'A'");
        assert_eq!(format_char(10), "'\\n'");
        assert_eq!(format_char(42), "'*'");
    }
}
