// SPDX-License-Identifier: GPL-2.0-or-later

use std::collections::HashMap;

use crate::args::ArgValue;
use crate::commands::{require_str, CmdError};
use crate::qmp::QmpConnection;

#[allow(deprecated)]
async fn commit_device(conn: &QmpConnection, device: &str) -> Result<(), CmdError> {
    conn.execute(qapi::qmp::block_commit {
        device: device.to_string(),
        job_id: None,
        base_node: None,
        base: None,
        top_node: None,
        top: None,
        backing_file: None,
        backing_mask_protocol: None,
        speed: None,
        on_error: None,
        filter_node_name: None,
        auto_finalize: None,
        auto_dismiss: None,
    })
    .await
    .map_err(CmdError::from)?;
    Ok(())
}

pub async fn cmd_commit(
    conn: &QmpConnection,
    args: &HashMap<String, ArgValue>,
) -> Result<String, CmdError> {
    let device = require_str(args, "device")?;

    if device == "all" {
        let blocks = conn
            .execute(qapi::qmp::query_block {})
            .await
            .map_err(CmdError::from)?;
        let mut errors = Vec::new();
        for block in &blocks {
            if block.inserted.is_some() {
                if let Err(e) = commit_device(conn, &block.device).await {
                    match e {
                        CmdError::Disconnected => return Err(CmdError::Disconnected),
                        CmdError::Command(msg) => {
                            errors.push(format!("'commit' error for '{}': {msg}", block.device));
                        }
                    }
                }
            }
        }
        if errors.is_empty() {
            Ok(String::new())
        } else {
            Err(CmdError::Command(errors.join("\n")))
        }
    } else {
        commit_device(conn, &device).await?;
        Ok(String::new())
    }
}
