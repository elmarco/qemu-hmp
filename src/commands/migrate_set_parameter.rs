// SPDX-License-Identifier: GPL-2.0-or-later

use std::collections::HashMap;

use qapi::Enum;
use qapi_qmp::MigrationParameter;

use crate::args::{parse_size, ArgValue};
use crate::commands::{require_str, CmdError};
use crate::qmp::QmpConnection;

/// Parse a boolean from a string, matching QEMU's visitor conventions.
fn parse_bool_value(s: &str) -> Result<bool, CmdError> {
    match s {
        "on" | "yes" | "true" | "y" | "1" => Ok(true),
        "off" | "no" | "false" | "n" | "0" => Ok(false),
        _ => Err(CmdError::Command(format!(
            "'{s}' is not a valid boolean value"
        ))),
    }
}

/// Parse an integer from a string (decimal or 0x hex).
fn parse_int_value(s: &str) -> Result<u64, CmdError> {
    let val = if let Some(hex) = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
        u64::from_str_radix(hex, 16)
    } else {
        s.parse::<u64>()
    };
    val.map_err(|_| CmdError::Command(format!("invalid parameter value: {s}")))
}

/// Parse a size value with optional K/M/G/T suffix (default unit: bytes).
fn parse_size_value(s: &str) -> Result<u64, CmdError> {
    parse_size(s).map(|v| v as u64).map_err(CmdError::Command)
}

/// Parse a size value with MiB as the default unit (matching qemu_strtosz_MiB).
fn parse_size_mib(s: &str) -> Result<u64, CmdError> {
    // If the string has a suffix, use normal size parsing.
    // Otherwise, treat as MiB.
    let last = s.bytes().last().unwrap_or(b'0');
    if last.is_ascii_alphabetic() {
        parse_size_value(s)
    } else {
        let mib = parse_int_value(s)?;
        Ok(mib.saturating_mul(1024 * 1024))
    }
}

pub async fn cmd_migrate_set_parameter(
    conn: &QmpConnection,
    args: &HashMap<String, ArgValue>,
) -> Result<String, CmdError> {
    let param_str = require_str(args, "parameter")?;
    let value_str = require_str(args, "value")?;

    let param = MigrationParameter::from_name(&param_str)
        .ok_or_else(|| CmdError::Command(format!("invalid parameter value: {param_str}")))?;

    if matches!(param, MigrationParameter::block_bitmap_mapping) {
        return Err(CmdError::Command(
            "The block-bitmap-mapping parameter can only be set through QMP".to_string(),
        ));
    }

    // Build a JSON object with a single key matching the QAPI parameter
    // name, then deserialize into MigrateSetParameters so that serde
    // handles the type mapping.
    let key = param.name();
    let json_val: serde_json::Value = match param {
        // Boolean parameters
        MigrationParameter::cpu_throttle_tailslow | MigrationParameter::direct_io => {
            serde_json::Value::Bool(parse_bool_value(&value_str)?)
        }

        // String parameters (StrOrNull in QMP)
        MigrationParameter::tls_creds
        | MigrationParameter::tls_hostname
        | MigrationParameter::tls_authz => serde_json::Value::String(value_str),

        // Enum parameters (passed as strings, QMP validates)
        MigrationParameter::multifd_compression
        | MigrationParameter::zero_page_detection
        | MigrationParameter::mode => serde_json::Value::String(value_str),

        // Size parameters with MiB default (matching qemu_strtosz_MiB)
        MigrationParameter::max_bandwidth | MigrationParameter::avail_switchover_bandwidth => {
            serde_json::Value::Number(parse_size_mib(&value_str)?.into())
        }

        // Size parameters with byte default (matching visit_type_size)
        MigrationParameter::downtime_limit
        | MigrationParameter::max_postcopy_bandwidth
        | MigrationParameter::announce_initial
        | MigrationParameter::announce_max
        | MigrationParameter::announce_rounds
        | MigrationParameter::announce_step
        | MigrationParameter::x_vcpu_dirty_limit_period
        | MigrationParameter::vcpu_dirty_limit
        | MigrationParameter::xbzrle_cache_size => {
            serde_json::Value::Number(parse_size_value(&value_str)?.into())
        }

        // Plain integer parameters
        _ => serde_json::Value::Number(parse_int_value(&value_str)?.into()),
    };

    let json_obj = serde_json::json!({ key: json_val });
    let params: qapi_qmp::MigrateSetParameters = serde_json::from_value(json_obj)
        .map_err(|e| CmdError::Command(format!("invalid parameter value: {e}")))?;

    conn.execute(qapi_qmp::migrate_set_parameters(params))
        .await
        .map_err(CmdError::from)?;

    Ok(String::new())
}
