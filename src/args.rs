// SPDX-License-Identifier: GPL-2.0-or-later

use std::collections::HashMap;

/// The type of an HMP command argument, derived from the single-char codes
/// in the `args_type` field of .hx command entries.
#[derive(Debug, Clone, PartialEq)]
pub enum ArgType {
    /// `s` or `S` — a string
    Str,
    /// `B` — block device name (treated as string)
    BlockDevice,
    /// `o` — size with optional K/M/G/T suffix
    Size,
    /// `i` — integer
    Int,
    /// `l` — long integer (supports 0x hex)
    Long,
    /// `F` — filename (treated as string)
    Filename,
    /// `M` — mebibytes (plain integer, converted to bytes by × 1048576)
    Mebibytes,
    /// `b` — boolean (on/off/true/false/1/0)
    Bool,
    /// `O` — object spec (treated as string)
    Object,
    /// `/` — format specifier (treated as string)
    Format,
    /// `i.` — optional integer preceded by `.` on the command line
    DotInt,
    /// `-X` — boolean flag, e.g. `-f`
    Flag(String),
    /// `-Xs` — flag that takes a string value, e.g. `-f png`
    FlagStr(String),
}

/// Definition of a single argument parsed from an `args_type` spec.
#[derive(Debug, Clone, PartialEq)]
pub struct ArgDef {
    pub name: String,
    pub arg_type: ArgType,
    pub optional: bool,
}

/// A parsed argument value ready for use at runtime.
#[derive(Debug, Clone, PartialEq)]
pub enum ArgValue {
    Str(String),
    Int(i64),
    Bool(bool),
}

/// Parse a decimal or `0x`-prefixed hexadecimal integer.
pub fn parse_int(s: &str) -> Result<i64, String> {
    if let Some(hex) = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
        i64::from_str_radix(hex, 16).map_err(|e| format!("invalid hex integer '{}': {}", s, e))
    } else {
        s.parse::<i64>()
            .map_err(|e| format!("invalid integer '{}': {}", s, e))
    }
}

/// Parse a size value with an optional single-letter suffix:
/// K = 1024, M = 1024^2, G = 1024^3, T = 1024^4.
/// Plain numbers (no suffix) are returned as-is.
pub fn parse_size(s: &str) -> Result<i64, String> {
    if s.is_empty() {
        return Err("empty size string".to_string());
    }

    let last = s.as_bytes()[s.len() - 1];
    let (num_part, multiplier) = match last {
        b'K' | b'k' => (&s[..s.len() - 1], 1024_i64),
        b'M' | b'm' => (&s[..s.len() - 1], 1024_i64 * 1024),
        b'G' | b'g' => (&s[..s.len() - 1], 1024_i64 * 1024 * 1024),
        b'T' | b't' => (&s[..s.len() - 1], 1024_i64 * 1024 * 1024 * 1024),
        _ => (s, 1_i64),
    };

    let base = parse_int(num_part)?;
    base.checked_mul(multiplier)
        .ok_or_else(|| format!("size overflow for '{}'", s))
}

fn parse_bool(s: &str) -> Result<bool, String> {
    match s.to_lowercase().as_str() {
        "on" | "true" | "1" => Ok(true),
        "off" | "false" | "0" => Ok(false),
        _ => Err(format!(
            "invalid boolean '{}': expected on/off/true/false/1/0",
            s
        )),
    }
}

/// Parse the `args_type` spec string from an .hx entry into a list of [`ArgDef`]s.
///
/// The spec is a comma-separated list of `name:type_code` pairs.  An empty
/// string yields an empty vec.  Type codes are documented on [`ArgType`].
pub fn parse_arg_defs(spec: &str) -> Result<Vec<ArgDef>, String> {
    if spec.is_empty() {
        return Ok(Vec::new());
    }

    spec.split(',')
        .map(|part| {
            let part = part.trim();
            // Split on the first ':'
            let (name, type_spec) = part
                .split_once(':')
                .ok_or_else(|| format!("invalid arg spec component (missing ':'): '{}'", part))?;

            // Check for trailing '?' (optional)
            let (type_spec, optional) = if let Some(stripped) = type_spec.strip_suffix('?') {
                (stripped, true)
            } else {
                (type_spec, false)
            };

            let arg_type = match type_spec {
                "s" | "S" => ArgType::Str,
                "B" => ArgType::BlockDevice,
                "o" => ArgType::Size,
                "i" => ArgType::Int,
                "l" => ArgType::Long,
                "M" => ArgType::Mebibytes,
                "F" => ArgType::Filename,
                "b" => ArgType::Bool,
                "O" => ArgType::Object,
                "/" => ArgType::Format,
                "i." => ArgType::DotInt,
                _ if type_spec.starts_with('-')
                    && type_spec.ends_with('s')
                    && type_spec.len() == 3 =>
                {
                    // Flag with string value: "-fs" means flag `-f` that takes a string.
                    // On the command line: `-f <value>`.
                    let flag = type_spec[..2].to_string(); // e.g. "-f"
                    ArgType::FlagStr(flag)
                }
                _ if type_spec.starts_with('-') => {
                    // Boolean flag: "-f" matches the literal token `-f`.
                    ArgType::Flag(type_spec.to_string())
                }
                other => {
                    eprintln!(
                        "warning: unknown arg type code '{}', treating as string",
                        other
                    );
                    ArgType::Str
                }
            };

            Ok(ArgDef {
                name: name.to_string(),
                arg_type,
                optional,
            })
        })
        .collect()
}

/// Match user input tokens against argument definitions and produce a map
/// of argument name to parsed value.
///
/// Flags are position-independent: each flag definition scans the entire
/// input for its flag string.  The first matching token is consumed and
/// removed; remaining tokens are then assigned to positional arguments
/// in definition order.
///
/// Required arguments that cannot be satisfied cause an error.  Optional
/// arguments that have no remaining input are simply omitted from the result.
pub fn parse_args(input: &[&str], defs: &[ArgDef]) -> Result<HashMap<String, ArgValue>, String> {
    let mut result = HashMap::new();

    // First pass: extract flags from anywhere in the token list.
    let mut remaining: Vec<&str> = input.to_vec();
    for def in defs {
        match &def.arg_type {
            ArgType::Flag(ref flag_str) => {
                if let Some(idx) = remaining.iter().position(|t| *t == flag_str.as_str()) {
                    result.insert(def.name.clone(), ArgValue::Bool(true));
                    remaining.remove(idx);
                } else {
                    result.insert(def.name.clone(), ArgValue::Bool(false));
                }
            }
            ArgType::FlagStr(ref flag_str) => {
                if let Some(idx) = remaining.iter().position(|t| *t == flag_str.as_str()) {
                    remaining.remove(idx);
                    if idx < remaining.len() {
                        let val = remaining.remove(idx);
                        result.insert(def.name.clone(), ArgValue::Str(val.to_string()));
                    } else {
                        return Err(format!("flag '{}' requires a value", flag_str));
                    }
                }
                // If flag not present, simply omit from result (it's optional).
            }
            _ => {}
        }
    }

    // Second pass: consume positional arguments from remaining tokens.
    let mut pos = 0;
    for (def_idx, def) in defs.iter().enumerate() {
        if matches!(def.arg_type, ArgType::Flag(_) | ArgType::FlagStr(_)) {
            continue; // Already handled above
        }

        if pos >= remaining.len() {
            // DotInt is implicitly optional (presence is determined by
            // a leading '.' token, not by the '?' suffix).
            if def.optional || matches!(def.arg_type, ArgType::DotInt) {
                continue;
            } else {
                return Err(format!("missing required argument '{}'", def.name));
            }
        }

        let token = remaining[pos];

        // Format type is special: only consume the token if it starts
        // with '/'.  Otherwise skip this argument and leave the token
        // for the next positional argument (QEMU uses the last-used
        // format as default when no '/' spec is present).
        if matches!(def.arg_type, ArgType::Format) && !token.starts_with('/') {
            continue;
        }

        // DotInt: optional integer preceded by '.'.  If the current
        // token is not '.', skip this argument entirely.
        if matches!(def.arg_type, ArgType::DotInt) {
            if token == "." {
                pos += 1; // consume '.'
                if pos >= remaining.len() {
                    return Err(format!(
                        "missing integer value after '.' for '{}'",
                        def.name
                    ));
                }
                let int_token = remaining[pos];
                pos += 1;
                let val = parse_int(int_token)?;
                result.insert(def.name.clone(), ArgValue::Int(val));
            }
            // If token != '.', skip — the argument is not present.
            continue;
        }

        pos += 1;

        let value = match &def.arg_type {
            ArgType::Str
            | ArgType::BlockDevice
            | ArgType::Filename
            | ArgType::Object
            | ArgType::Format => ArgValue::Str(token.to_string()),
            ArgType::Int => ArgValue::Int(parse_int(token)?),
            ArgType::Long => {
                // Long type stores the raw expression string.
                // When this is the last positional arg, consume all
                // remaining tokens (to support multi-token expressions
                // like "1 + 2").  Otherwise consume a single token so
                // subsequent args can be parsed.
                let has_more_positional = defs[def_idx + 1..]
                    .iter()
                    .any(|d| !matches!(d.arg_type, ArgType::Flag(_) | ArgType::FlagStr(_)));
                let expr = if !has_more_positional && pos < remaining.len() {
                    let mut parts = vec![token];
                    parts.extend_from_slice(&remaining[pos..]);
                    pos = remaining.len();
                    parts.join(" ")
                } else {
                    token.to_string()
                };
                ArgValue::Str(expr)
            }
            ArgType::Size => ArgValue::Int(parse_size(token)?),
            ArgType::Mebibytes => {
                let val = parse_int(token)?;
                if val < 0 {
                    return Err("enter a positive value".to_string());
                }
                ArgValue::Int(
                    val.checked_mul(1024 * 1024)
                        .ok_or_else(|| format!("value overflow for '{}' MiB", token))?,
                )
            }
            ArgType::Bool => ArgValue::Bool(parse_bool(token)?),
            ArgType::DotInt | ArgType::Flag(_) | ArgType::FlagStr(_) => unreachable!(),
        };

        result.insert(def.name.clone(), value);
    }

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_spec() {
        let defs = parse_arg_defs("").unwrap();
        assert!(defs.is_empty());
        let result = parse_args(&[], &defs).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn single_required_string() {
        let defs = parse_arg_defs("device:B").unwrap();
        assert_eq!(defs.len(), 1);
        assert_eq!(defs[0].name, "device");
        assert_eq!(defs[0].arg_type, ArgType::BlockDevice);
        assert!(!defs[0].optional);

        let result = parse_args(&["virtio0"], &defs).unwrap();
        assert_eq!(result.get("device"), Some(&ArgValue::Str("virtio0".into())));
    }

    #[test]
    fn missing_required_arg_error() {
        let defs = parse_arg_defs("device:B").unwrap();
        let err = parse_args(&[], &defs).unwrap_err();
        assert!(err.contains("missing required argument 'device'"));
    }

    #[test]
    fn optional_missing_arg() {
        let defs = parse_arg_defs("name:S?").unwrap();
        assert!(defs[0].optional);

        let result = parse_args(&[], &defs).unwrap();
        assert!(!result.contains_key("name"));
    }

    #[test]
    fn optional_present_arg() {
        let defs = parse_arg_defs("name:S?").unwrap();
        let result = parse_args(&["hello"], &defs).unwrap();
        assert_eq!(result.get("name"), Some(&ArgValue::Str("hello".into())));
    }

    #[test]
    fn flag_present() {
        let defs = parse_arg_defs("force:-f,device:B").unwrap();
        let result = parse_args(&["-f", "virtio0"], &defs).unwrap();
        assert_eq!(result.get("force"), Some(&ArgValue::Bool(true)));
        assert_eq!(result.get("device"), Some(&ArgValue::Str("virtio0".into())));
    }

    #[test]
    fn flag_absent() {
        let defs = parse_arg_defs("force:-f,device:B").unwrap();
        let result = parse_args(&["virtio0"], &defs).unwrap();
        assert_eq!(result.get("force"), Some(&ArgValue::Bool(false)));
        assert_eq!(result.get("device"), Some(&ArgValue::Str("virtio0".into())));
    }

    #[test]
    fn flag_after_positional() {
        // Flags are position-independent: -f after the device still works.
        let defs = parse_arg_defs("force:-f,device:B").unwrap();
        let result = parse_args(&["virtio0", "-f"], &defs).unwrap();
        assert_eq!(result.get("force"), Some(&ArgValue::Bool(true)));
        assert_eq!(result.get("device"), Some(&ArgValue::Str("virtio0".into())));
    }

    #[test]
    fn size_with_suffix() {
        assert_eq!(parse_size("10G").unwrap(), 10 * 1024 * 1024 * 1024);
        assert_eq!(parse_size("512M").unwrap(), 512 * 1024 * 1024);
        assert_eq!(parse_size("1024").unwrap(), 1024);
        assert_eq!(parse_size("2K").unwrap(), 2048);
        assert_eq!(parse_size("1T").unwrap(), 1024_i64.pow(4));
    }

    #[test]
    fn boolean_on_off() {
        let defs = parse_arg_defs("state:b").unwrap();
        let on = parse_args(&["on"], &defs).unwrap();
        assert_eq!(on.get("state"), Some(&ArgValue::Bool(true)));

        let off = parse_args(&["off"], &defs).unwrap();
        assert_eq!(off.get("state"), Some(&ArgValue::Bool(false)));

        let t = parse_args(&["true"], &defs).unwrap();
        assert_eq!(t.get("state"), Some(&ArgValue::Bool(true)));

        let f = parse_args(&["false"], &defs).unwrap();
        assert_eq!(f.get("state"), Some(&ArgValue::Bool(false)));

        let one = parse_args(&["1"], &defs).unwrap();
        assert_eq!(one.get("state"), Some(&ArgValue::Bool(true)));

        let zero = parse_args(&["0"], &defs).unwrap();
        assert_eq!(zero.get("state"), Some(&ArgValue::Bool(false)));
    }

    #[test]
    fn long_stores_raw_expression() {
        let defs = parse_arg_defs("addr:l").unwrap();
        let result = parse_args(&["0xff"], &defs).unwrap();
        assert_eq!(result.get("addr"), Some(&ArgValue::Str("0xff".into())));
    }

    #[test]
    fn long_consumes_remaining_when_last() {
        // Last positional Long consumes all remaining tokens as one expression.
        let defs = parse_arg_defs("fmt:/,val:l").unwrap();
        let result = parse_args(&["/x", "1", "+", "2"], &defs).unwrap();
        assert_eq!(result.get("val"), Some(&ArgValue::Str("1 + 2".into())));
    }

    #[test]
    fn long_single_token_when_not_last() {
        // Long followed by more positional args consumes a single token.
        let defs = parse_arg_defs("val:l,size:i,filename:s").unwrap();
        let result = parse_args(&["0xb8000", "4096", "/tmp/file"], &defs).unwrap();
        assert_eq!(result.get("val"), Some(&ArgValue::Str("0xb8000".into())));
        assert_eq!(result.get("size"), Some(&ArgValue::Int(4096)));
        assert_eq!(
            result.get("filename"),
            Some(&ArgValue::Str("/tmp/file".into()))
        );
    }

    #[test]
    fn multiple_long_args() {
        // Multiple Long args each consume one token; last consumes remainder.
        let defs = parse_arg_defs("a:l,b:l,c:l").unwrap();
        let result = parse_args(&["10", "20", "30"], &defs).unwrap();
        assert_eq!(result.get("a"), Some(&ArgValue::Str("10".into())));
        assert_eq!(result.get("b"), Some(&ArgValue::Str("20".into())));
        assert_eq!(result.get("c"), Some(&ArgValue::Str("30".into())));
    }

    #[test]
    fn long_with_flags_between() {
        // Flags don't count as positional — Long before another Long
        // should consume one token even with flags in between.
        let defs = parse_arg_defs("flag:-b,x:l,y:l").unwrap();
        let result = parse_args(&["-b", "100", "200"], &defs).unwrap();
        assert_eq!(result.get("flag"), Some(&ArgValue::Bool(true)));
        assert_eq!(result.get("x"), Some(&ArgValue::Str("100".into())));
        assert_eq!(result.get("y"), Some(&ArgValue::Str("200".into())));
    }

    #[test]
    fn mixed_device_and_size() {
        // Mimics block_resize: device:B,size:o
        let defs = parse_arg_defs("device:B,size:o").unwrap();
        let result = parse_args(&["virtio0", "10G"], &defs).unwrap();
        assert_eq!(result.get("device"), Some(&ArgValue::Str("virtio0".into())));
        assert_eq!(
            result.get("size"),
            Some(&ArgValue::Int(10 * 1024 * 1024 * 1024))
        );
    }

    #[test]
    fn flag_with_string_value() {
        // `-fs` means flag `-f` that takes a string value.
        let defs = parse_arg_defs("format:-fs,filename:F").unwrap();
        assert!(matches!(defs[0].arg_type, ArgType::FlagStr(ref s) if s == "-f"));

        // Flag present with value
        let result = parse_args(&["out.ppm", "-f", "png"], &defs).unwrap();
        assert_eq!(result.get("format"), Some(&ArgValue::Str("png".into())));
        assert_eq!(
            result.get("filename"),
            Some(&ArgValue::Str("out.ppm".into()))
        );

        // Flag absent — omitted from result
        let result = parse_args(&["out.ppm"], &defs).unwrap();
        assert!(!result.contains_key("format"));
        assert_eq!(
            result.get("filename"),
            Some(&ArgValue::Str("out.ppm".into()))
        );
    }

    #[test]
    fn flag_str_before_positional() {
        // Flag with string value before positional args
        let defs = parse_arg_defs("format:-fs,filename:F,device:s?").unwrap();
        let result = parse_args(&["-f", "png", "out.ppm"], &defs).unwrap();
        assert_eq!(result.get("format"), Some(&ArgValue::Str("png".into())));
        assert_eq!(
            result.get("filename"),
            Some(&ArgValue::Str("out.ppm".into()))
        );
        assert!(!result.contains_key("device"));
    }

    #[test]
    fn flag_str_missing_value_error() {
        // Flag present but no value after it
        let defs = parse_arg_defs("format:-fs,filename:F").unwrap();
        let err = parse_args(&["out.ppm", "-f"], &defs).unwrap_err();
        assert!(err.contains("requires a value"));
    }

    #[test]
    fn format_specifier() {
        let defs = parse_arg_defs("fmt:/").unwrap();
        let result = parse_args(&["/x"], &defs).unwrap();
        assert_eq!(result.get("fmt"), Some(&ArgValue::Str("/x".into())));
    }

    #[test]
    fn mebibytes_type() {
        let defs = parse_arg_defs("value:M").unwrap();
        assert_eq!(defs.len(), 1);
        assert_eq!(defs[0].arg_type, ArgType::Mebibytes);

        let result = parse_args(&["256"], &defs).unwrap();
        assert_eq!(result.get("value"), Some(&ArgValue::Int(256 * 1024 * 1024)));
    }

    #[test]
    fn mebibytes_rejects_negative() {
        let defs = parse_arg_defs("value:M").unwrap();
        let err = parse_args(&["-1"], &defs).unwrap_err();
        assert!(err.contains("positive"));
    }

    #[test]
    fn parse_int_decimal_and_hex() {
        assert_eq!(parse_int("42").unwrap(), 42);
        assert_eq!(parse_int("0x1a").unwrap(), 26);
        assert_eq!(parse_int("0XFF").unwrap(), 255);
        assert_eq!(parse_int("-1").unwrap(), -1);
        assert!(parse_int("notanumber").is_err());
    }

    #[test]
    fn mixed_required_optional_defs() {
        let defs = parse_arg_defs("device:B,speed:o?,base:s?").unwrap();
        assert_eq!(defs.len(), 3);
        assert_eq!(defs[0].name, "device");
        assert_eq!(defs[0].arg_type, ArgType::BlockDevice);
        assert!(!defs[0].optional);
        assert_eq!(defs[1].name, "speed");
        assert_eq!(defs[1].arg_type, ArgType::Size);
        assert!(defs[1].optional);
        assert_eq!(defs[2].name, "base");
        assert_eq!(defs[2].arg_type, ArgType::Str);
        assert!(defs[2].optional);
    }

    #[test]
    fn many_flags_complex_spec() {
        // From dump-guest-memory
        let defs = parse_arg_defs(
            "paging:-p,detach:-d,windmp:-w,zlib:-z,lzo:-l,snappy:-s,raw:-R,filename:F,begin:l?,length:l?",
        )
        .unwrap();
        assert_eq!(defs.len(), 10);
        assert_eq!(defs[0].name, "paging");
        assert!(matches!(defs[0].arg_type, ArgType::Flag(ref s) if s == "-p"));
        assert!(!defs[0].optional); // flags get optional from parse_args, not parse_arg_defs
        assert_eq!(defs[7].name, "filename");
        assert_eq!(defs[7].arg_type, ArgType::Filename);
        assert!(!defs[7].optional);
        assert_eq!(defs[8].name, "begin");
        assert_eq!(defs[8].arg_type, ArgType::Long);
        assert!(defs[8].optional);
        assert_eq!(defs[9].name, "length");
        assert_eq!(defs[9].arg_type, ArgType::Long);
        assert!(defs[9].optional);
    }

    #[test]
    fn malformed_spec_missing_colon() {
        let err = parse_arg_defs("device").unwrap_err();
        assert!(err.contains("missing ':'"));
    }

    #[test]
    fn dot_int_arg_type() {
        // "i." is used in hmp-commands.hx for I/O port read
        let defs = parse_arg_defs("fmt:/,addr:i,index:i.").unwrap();
        assert_eq!(defs.len(), 3);
        assert_eq!(defs[2].name, "index");
        assert_eq!(defs[2].arg_type, ArgType::DotInt);
    }

    #[test]
    fn dot_int_present() {
        let defs = parse_arg_defs("fmt:/,addr:i,index:i.").unwrap();
        let result = parse_args(&["/b", "0x61", ".", "5"], &defs).unwrap();
        assert_eq!(result.get("addr"), Some(&ArgValue::Int(0x61)));
        assert_eq!(result.get("index"), Some(&ArgValue::Int(5)));
    }

    #[test]
    fn dot_int_absent() {
        let defs = parse_arg_defs("fmt:/,addr:i,index:i.").unwrap();
        let result = parse_args(&["/b", "0x61"], &defs).unwrap();
        assert_eq!(result.get("addr"), Some(&ArgValue::Int(0x61)));
        assert!(!result.contains_key("index"));
    }
}
