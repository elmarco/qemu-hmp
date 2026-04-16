// SPDX-License-Identifier: GPL-2.0-or-later

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::args::ArgValue;
use crate::commands::{require_int, CmdError};
use crate::qmp::QmpConnection;

// x-ioport-read / x-ioport-write are not yet in the qapi-rs crate,
// define them locally.

#[derive(Debug, Deserialize)]
pub struct IoPortReadInfo {
    pub value: i64,
}

#[derive(Debug, Serialize)]
#[allow(non_camel_case_types)]
pub struct x_ioport_read {
    pub addr: i64,
    pub size: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub index: Option<i64>,
}

impl qapi_qmp::QmpCommand for x_ioport_read {}
impl qapi::Command for x_ioport_read {
    const NAME: &'static str = "x-ioport-read";
    const ALLOW_OOB: bool = false;
    type Ok = IoPortReadInfo;
}

#[derive(Debug, Serialize)]
#[allow(non_camel_case_types)]
pub struct x_ioport_write {
    pub addr: i64,
    pub size: i64,
    pub val: i64,
}

impl qapi_qmp::QmpCommand for x_ioport_write {}
impl qapi::Command for x_ioport_write {
    const NAME: &'static str = "x-ioport-write";
    const ALLOW_OOB: bool = false;
    type Ok = qapi::Empty;
}

/// Extract the access size from the `/fmt` argument.
/// Size specifiers: b=1, h=2, w=4 (default).
fn get_size(args: &HashMap<String, ArgValue>) -> i64 {
    if let Some(ArgValue::Str(s)) = args.get("fmt") {
        let s = s.strip_prefix('/').unwrap_or(s);
        for c in s.chars() {
            match c {
                'b' => return 1,
                'h' => return 2,
                'w' => return 4,
                _ => {}
            }
        }
    }
    4 // default: word (4 bytes)
}

pub async fn cmd_ioport_read(
    conn: &QmpConnection,
    args: &HashMap<String, ArgValue>,
) -> Result<String, CmdError> {
    let size = get_size(args);
    let addr = require_int(args, "addr")?;
    let index = match args.get("index") {
        Some(ArgValue::Int(n)) => Some(*n),
        _ => None,
    };

    let info = conn
        .execute(x_ioport_read { addr, size, index })
        .await
        .map_err(CmdError::from)?;

    // Compute effective address for display (mirrors C logic)
    let mut display_addr = addr;
    if index.is_some() {
        display_addr += 1;
    }
    display_addr &= 0xffff;

    let suffix = match size {
        2 => 'w',
        4 => 'l',
        _ => 'b',
    };

    Ok(format!(
        "port{}[0x{:04x}] = 0x{:0width$x}",
        suffix,
        display_addr,
        info.value as u32,
        width = size as usize * 2
    ))
}

pub async fn cmd_ioport_write(
    conn: &QmpConnection,
    args: &HashMap<String, ArgValue>,
) -> Result<String, CmdError> {
    let size = get_size(args);
    let addr = require_int(args, "addr")?;
    let val = require_int(args, "val")?;

    conn.execute(x_ioport_write { addr, size, val })
        .await
        .map_err(CmdError::from)?;

    Ok(String::new())
}
