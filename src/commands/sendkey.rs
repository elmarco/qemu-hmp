// SPDX-License-Identifier: GPL-2.0-or-later

use std::collections::HashMap;

use qapi_qmp::{IntWrapper, KeyValue, QKeyCode, QKeyCodeWrapper};

use crate::args::ArgValue;
use crate::commands::CmdError;
use crate::qmp::QmpConnection;

/// Parse a key name string into a list of `KeyValue` entries.
///
/// Keys are separated by `-`.  Each key is either:
/// - `<` — alias for `less`
/// - `0x...` — raw scancode number
/// - A `QKeyCode` name (e.g. `ctrl`, `alt`, `f1`, `ret`)
fn parse_keys(keys_str: &str) -> Result<Vec<KeyValue>, String> {
    let mut keys = Vec::new();

    for part in keys_str.split('-') {
        // Be compatible with old interface: convert "<" to "less"
        let name = if part == "<" { "less" } else { part };

        let kv = if let Some(hex) = name.strip_prefix("0x") {
            let value =
                i64::from_str_radix(hex, 16).map_err(|_| format!("invalid parameter: {part}"))?;
            KeyValue::number(IntWrapper { data: value })
        } else {
            let qcode: QKeyCode = name
                .parse()
                .map_err(|_| format!("invalid parameter: {part}"))?;
            KeyValue::qcode(QKeyCodeWrapper { data: qcode })
        };
        keys.push(kv);
    }

    Ok(keys)
}

pub async fn cmd_sendkey(
    conn: &QmpConnection,
    args: &HashMap<String, ArgValue>,
) -> Result<String, CmdError> {
    let keys_str = match args.get("keys") {
        Some(ArgValue::Str(s)) => s.as_str(),
        _ => return Err(CmdError::Command("missing required argument 'keys'".into())),
    };

    let keys = parse_keys(keys_str).map_err(CmdError::Command)?;

    let hold_time = match args.get("hold-time") {
        Some(ArgValue::Int(n)) => Some(*n),
        _ => None,
    };

    conn.execute(qapi_qmp::send_key { keys, hold_time })
        .await
        .map_err(CmdError::from)?;

    Ok(String::new())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_single_key() {
        let keys = parse_keys("ret").unwrap();
        assert_eq!(keys.len(), 1);
        assert!(matches!(&keys[0], KeyValue::qcode(w) if w.data == QKeyCode::ret));
    }

    #[test]
    fn parse_combo() {
        let keys = parse_keys("ctrl-alt-f1").unwrap();
        assert_eq!(keys.len(), 3);
        assert!(matches!(&keys[0], KeyValue::qcode(w) if w.data == QKeyCode::ctrl));
        assert!(matches!(&keys[1], KeyValue::qcode(w) if w.data == QKeyCode::alt));
        assert!(matches!(&keys[2], KeyValue::qcode(w) if w.data == QKeyCode::f1));
    }

    #[test]
    fn parse_hex_scancode() {
        let keys = parse_keys("0x1c").unwrap();
        assert_eq!(keys.len(), 1);
        assert!(matches!(&keys[0], KeyValue::number(w) if w.data == 0x1c));
    }

    #[test]
    fn parse_less_alias() {
        let keys = parse_keys("<").unwrap();
        assert_eq!(keys.len(), 1);
        assert!(matches!(&keys[0], KeyValue::qcode(w) if w.data == QKeyCode::less));
    }

    #[test]
    fn parse_invalid_key() {
        let err = parse_keys("nonexistent").unwrap_err();
        assert_eq!(err, "invalid parameter: nonexistent");
    }

    #[test]
    fn parse_mixed_combo() {
        let keys = parse_keys("ctrl-0x1c").unwrap();
        assert_eq!(keys.len(), 2);
        assert!(matches!(&keys[0], KeyValue::qcode(w) if w.data == QKeyCode::ctrl));
        assert!(matches!(&keys[1], KeyValue::number(w) if w.data == 0x1c));
    }
}
