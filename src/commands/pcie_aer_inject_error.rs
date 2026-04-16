// SPDX-License-Identifier: GPL-2.0-or-later

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::args::ArgValue;
use crate::commands::{opt_bool, require_str, CmdError};
use crate::qmp::QmpConnection;

// x-pcie-aer-inject-error is an experimental QMP command added for
// the external HMP.  We define a raw struct since qapi-rs doesn't
// have generated bindings for it yet.

#[derive(Debug, Serialize)]
#[allow(non_camel_case_types)]
pub struct raw_x_pcie_aer_inject_error {
    pub id: String,
    #[serde(rename = "error-status")]
    pub error_status: String,
    #[serde(rename = "correctable", skip_serializing_if = "Option::is_none")]
    pub correctable: Option<bool>,
    #[serde(rename = "advisory-non-fatal", skip_serializing_if = "Option::is_none")]
    pub advisory_non_fatal: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub header0: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub header1: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub header2: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub header3: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prefix0: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prefix1: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prefix2: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prefix3: Option<i64>,
}

#[derive(Debug, Deserialize)]
#[allow(non_camel_case_types, dead_code)]
pub struct PCIEAERInjectedError {
    pub id: String,
    #[serde(rename = "root-bus")]
    pub root_bus: String,
    pub bus: i64,
    pub slot: i64,
    pub function: i64,
}

impl qapi_qmp::QmpCommand for raw_x_pcie_aer_inject_error {}
impl qapi::Command for raw_x_pcie_aer_inject_error {
    const NAME: &'static str = "x-pcie-aer-inject-error";
    const ALLOW_OOB: bool = false;
    type Ok = PCIEAERInjectedError;
}

pub async fn cmd_pcie_aer_inject_error(
    conn: &QmpConnection,
    args: &HashMap<String, ArgValue>,
) -> Result<String, CmdError> {
    let id = require_str(args, "id")?;
    let error_status = require_str(args, "error_status")?;
    let advisory_non_fatal = opt_bool(args, "advisory_non_fatal");
    let correctable = opt_bool(args, "correctable");

    let header0 = opt_int(args, "header0");
    let header1 = opt_int(args, "header1");
    let header2 = opt_int(args, "header2");
    let header3 = opt_int(args, "header3");
    let prefix0 = opt_int(args, "prefix0");
    let prefix1 = opt_int(args, "prefix1");
    let prefix2 = opt_int(args, "prefix2");
    let prefix3 = opt_int(args, "prefix3");

    let result = conn
        .execute(raw_x_pcie_aer_inject_error {
            id: id.clone(),
            error_status,
            correctable: if correctable { Some(true) } else { None },
            advisory_non_fatal: if advisory_non_fatal { Some(true) } else { None },
            header0,
            header1,
            header2,
            header3,
            prefix0,
            prefix1,
            prefix2,
            prefix3,
        })
        .await
        .map_err(CmdError::from)?;

    // Format to match the built-in HMP output exactly:
    // "OK id: %s root bus: %s, bus: %x devfn: %x.%x\n"
    Ok(format!(
        "OK id: {} root bus: {}, bus: {:x} devfn: {:x}.{:x}\n",
        result.id, result.root_bus, result.bus, result.slot, result.function
    ))
}

/// Extract an optional integer argument.
fn opt_int(args: &HashMap<String, ArgValue>, name: &str) -> Option<i64> {
    match args.get(name) {
        Some(ArgValue::Int(n)) => Some(*n),
        _ => None,
    }
}
