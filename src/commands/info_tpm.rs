// SPDX-License-Identifier: GPL-2.0-or-later

use std::collections::HashMap;
use std::fmt::Write;

use qapi::Enum;

use crate::args::ArgValue;
use crate::commands::CmdError;
use crate::qmp::QmpConnection;

pub async fn cmd_info_tpm(
    conn: &QmpConnection,
    _args: &HashMap<String, ArgValue>,
) -> Result<String, CmdError> {
    let list = match conn.execute(qapi_qmp::query_tpm {}).await {
        Ok(list) => list,
        Err(_) => {
            return Ok("TPM device not supported\n".to_string());
        }
    };

    let mut out = String::new();
    if !list.is_empty() {
        writeln!(out, "TPM device:").unwrap();
    }

    for (c, ti) in list.iter().enumerate() {
        writeln!(out, " tpm{}: model={}", c, ti.model.name()).unwrap();

        let type_name = ti.options.type_().name();
        write!(out, "  \\ {}: type={}", ti.id, type_name).unwrap();

        match &ti.options {
            qapi_qmp::TpmTypeOptions::passthrough(w) => {
                let tpo = &w.data;
                if let Some(ref path) = tpo.path {
                    write!(out, ",path={}", path).unwrap();
                }
                if let Some(ref cancel_path) = tpo.cancel_path {
                    write!(out, ",cancel-path={}", cancel_path).unwrap();
                }
            }
            qapi_qmp::TpmTypeOptions::emulator(w) => {
                write!(out, ",chardev={}", w.data.chardev).unwrap();
            }
        }

        writeln!(out).unwrap();
    }

    Ok(out)
}
