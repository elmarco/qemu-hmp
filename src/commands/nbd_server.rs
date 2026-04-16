// SPDX-License-Identifier: GPL-2.0-or-later

#![allow(deprecated)] // nbd-server-add and nbd-server-remove are deprecated QMP commands

use std::collections::HashMap;

use crate::args::ArgValue;
use crate::commands::{opt_bool, require_str, CmdError};
use crate::qmp::QmpConnection;

/// Parse a URI string into a `SocketAddressLegacy`.
///
/// Handles the same formats as QEMU's `socket_parse()`:
///   - `host:port` — inet (IPv4 or hostname)
///   - `[host]:port` — inet (IPv6)
fn parse_uri(uri: &str) -> Result<qapi_qmp::SocketAddressLegacy, CmdError> {
    // IPv6: [host]:port
    if let Some(rest) = uri.strip_prefix('[') {
        let Some((host, port)) = rest.split_once("]:") else {
            return Err(CmdError::Command(format!("error parsing address '{uri}'")));
        };
        return Ok(qapi_qmp::SocketAddressLegacy::inet(
            qapi_qmp::InetSocketAddress::from(qapi_qmp::InetSocketAddressBase {
                host: host.to_string(),
                port: port.to_string(),
            })
            .into(),
        ));
    }

    // host:port (split on last ':' to handle hostnames with no ambiguity)
    let Some((host, port)) = uri.rsplit_once(':') else {
        return Err(CmdError::Command(format!("error parsing address '{uri}'")));
    };

    Ok(qapi_qmp::SocketAddressLegacy::inet(
        qapi_qmp::InetSocketAddress::from(qapi_qmp::InetSocketAddressBase {
            host: host.to_string(),
            port: port.to_string(),
        })
        .into(),
    ))
}

pub async fn cmd_nbd_server_start(
    conn: &QmpConnection,
    args: &HashMap<String, ArgValue>,
) -> Result<String, CmdError> {
    let uri = require_str(args, "uri")?;
    let writable = opt_bool(args, "writable");
    let all = opt_bool(args, "all");

    if writable && !all {
        return Err(CmdError::Command(
            "-w only valid together with -a".to_string(),
        ));
    }

    let addr = parse_uri(&uri)?;

    conn.execute(qapi_qmp::nbd_server_start {
        addr,
        max_connections: None,
        tls_authz: None,
        tls_creds: None,
    })
    .await
    .map_err(CmdError::from)?;

    if !all {
        return Ok(String::new());
    }

    // Export all block devices that have inserted media.
    let block_list = conn
        .execute(qapi_qmp::query_block {})
        .await
        .map_err(CmdError::from)?;

    for info in &block_list {
        if info.inserted.is_none() {
            continue;
        }

        let result = conn
            .execute(qapi_qmp::nbd_server_add(qapi_qmp::NbdServerAddOptions {
                device: info.device.clone(),
                base: qapi_qmp::BlockExportOptionsNbdBase {
                    name: None,
                    description: None,
                },
                writable: Some(writable),
                bitmap: None,
            }))
            .await;

        if let Err(e) = result {
            // On failure, stop the server and report the error.
            let _ = conn.execute(qapi_qmp::nbd_server_stop {}).await;
            return Err(CmdError::from(e));
        }
    }

    Ok(String::new())
}

pub async fn cmd_nbd_server_add(
    conn: &QmpConnection,
    args: &HashMap<String, ArgValue>,
) -> Result<String, CmdError> {
    let device = require_str(args, "device")?;
    let name = args.get("name").and_then(|v| {
        if let ArgValue::Str(s) = v {
            Some(s.clone())
        } else {
            None
        }
    });
    let writable = opt_bool(args, "writable");

    conn.execute(qapi_qmp::nbd_server_add(qapi_qmp::NbdServerAddOptions {
        device,
        base: qapi_qmp::BlockExportOptionsNbdBase {
            name,
            description: None,
        },
        writable: Some(writable),
        bitmap: None,
    }))
    .await
    .map_err(CmdError::from)?;

    Ok(String::new())
}

pub async fn cmd_nbd_server_remove(
    conn: &QmpConnection,
    args: &HashMap<String, ArgValue>,
) -> Result<String, CmdError> {
    let name = require_str(args, "name")?;
    let force = opt_bool(args, "force");

    // Match the C behavior: only send mode when -f is specified.
    // Rely on BLOCK_EXPORT_REMOVE_MODE_SAFE being the server default.
    let mode = if force {
        Some(qapi_qmp::BlockExportRemoveMode::hard)
    } else {
        None
    };

    conn.execute(qapi_qmp::nbd_server_remove { name, mode })
        .await
        .map_err(CmdError::from)?;

    Ok(String::new())
}

pub async fn cmd_nbd_server_stop(
    conn: &QmpConnection,
    _args: &HashMap<String, ArgValue>,
) -> Result<String, CmdError> {
    conn.execute(qapi_qmp::nbd_server_stop {})
        .await
        .map_err(CmdError::from)?;

    Ok(String::new())
}
