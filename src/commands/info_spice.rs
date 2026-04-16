// SPDX-License-Identifier: GPL-2.0-or-later

use std::collections::HashMap;
use std::fmt::Write;

use qapi::Enum;

use crate::args::ArgValue;
use crate::commands::CmdError;
use crate::qmp::QmpConnection;

/// Map SPICE channel type numbers to names, matching the C array in
/// ui/ui-hmp-cmds.c (indexed by the libspice SPICE_CHANNEL_* constants).
fn spice_channel_name(channel_type: i64) -> &'static str {
    match channel_type {
        1 => "main",
        2 => "display",
        3 => "inputs",
        4 => "cursor",
        5 => "playback",
        6 => "record",
        7 => "tunnel",
        8 => "smartcard",
        9 => "usbredir",
        10 => "port",
        11 => "webdav",
        _ => "unknown",
    }
}

pub async fn cmd_info_spice(
    conn: &QmpConnection,
    _args: &HashMap<String, ArgValue>,
) -> Result<String, CmdError> {
    let info = conn
        .execute(qapi_qmp::query_spice {})
        .await
        .map_err(CmdError::from)?;

    let mut out = String::new();

    if !info.enabled {
        writeln!(out, "Server: disabled").unwrap();
        return Ok(out);
    }

    writeln!(out, "Server:").unwrap();
    if let Some(port) = info.port {
        writeln!(
            out,
            "     address: {}:{}",
            info.host.as_deref().unwrap_or(""),
            port
        )
        .unwrap();
    }
    if let Some(tls_port) = info.tls_port {
        writeln!(
            out,
            "     address: {}:{} [tls]",
            info.host.as_deref().unwrap_or(""),
            tls_port
        )
        .unwrap();
    }
    writeln!(
        out,
        "    migrated: {}",
        if info.migrated { "true" } else { "false" }
    )
    .unwrap();
    writeln!(out, "        auth: {}", info.auth.as_deref().unwrap_or("")).unwrap();
    writeln!(
        out,
        "    compiled: {}",
        info.compiled_version.as_deref().unwrap_or("")
    )
    .unwrap();
    writeln!(out, "  mouse-mode: {}", info.mouse_mode.name()).unwrap();

    match info.channels {
        None => {
            writeln!(out, "Channels: none").unwrap();
        }
        Some(ref channels) if channels.is_empty() => {
            writeln!(out, "Channels: none").unwrap();
        }
        Some(ref channels) => {
            for chan in channels {
                writeln!(out, "Channel:").unwrap();
                writeln!(
                    out,
                    "     address: {}:{}{}",
                    chan.base.host,
                    chan.base.port,
                    if chan.tls { " [tls]" } else { "" }
                )
                .unwrap();
                writeln!(out, "     session: {}", chan.connection_id).unwrap();
                writeln!(
                    out,
                    "     channel: {}:{}",
                    chan.channel_type, chan.channel_id
                )
                .unwrap();
                writeln!(
                    out,
                    "     channel name: {}",
                    spice_channel_name(chan.channel_type)
                )
                .unwrap();
            }
        }
    }

    Ok(out)
}
