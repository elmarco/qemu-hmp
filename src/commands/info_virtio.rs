// SPDX-License-Identifier: GPL-2.0-or-later

use std::collections::HashMap;
use std::fmt::Write;

use crate::args::ArgValue;
use crate::commands::{require_int, require_str, CmdError};
use crate::qmp::QmpConnection;

/// Format a tab-separated, comma-joined list of strings.
/// Matches the pattern used by hmp_virtio_dump_status/features/protocols:
///   \titem1,\n\titem2,\n\titemN\n
fn fmt_tab_list(out: &mut String, items: &[String]) {
    for (i, item) in items.iter().enumerate() {
        write!(out, "\t{item}").unwrap();
        if i + 1 < items.len() {
            writeln!(out, ",").unwrap();
        }
    }
    writeln!(out).unwrap();
}

fn fmt_virtio_dump_status(out: &mut String, status: &qapi_qmp::VirtioDeviceStatus) {
    fmt_tab_list(out, &status.statuses);
    if let Some(unknown) = status.unknown_statuses {
        // C uses PRIx32 with %016 — but it's a u8 in qapi-qmp (u32 in QAPI schema)
        writeln!(out, "  unknown-statuses(0x{unknown:016x})").unwrap();
    }
}

fn fmt_virtio_dump_features(out: &mut String, features: &qapi_qmp::VirtioDeviceFeatures) {
    fmt_tab_list(out, &features.transports);
    if let Some(ref dev_features) = features.dev_features {
        if !dev_features.is_empty() {
            fmt_tab_list(out, dev_features);
        }
    }
    if let Some(unknown) = features.unknown_dev_features {
        // TODO: qapi-qmp 0.15 lacks unknown_dev_features2 (added in QEMU 10.2).
        // The C handler prints both as a concatenated 128-bit hex value.
        // Once qapi-rs adds the field, use it here instead of 0.
        writeln!(out, "  unknown-features(0x{:016x}{unknown:016x})", 0u64).unwrap();
    }
}

fn fmt_virtio_dump_protocols(out: &mut String, pcol: &qapi_qmp::VhostDeviceProtocols) {
    fmt_tab_list(out, &pcol.protocols);
    if let Some(unknown) = pcol.unknown_protocols {
        writeln!(out, "  unknown-protocols(0x{unknown:016x})").unwrap();
    }
}

pub async fn cmd_info_virtio(
    conn: &QmpConnection,
    _args: &HashMap<String, ArgValue>,
) -> Result<String, CmdError> {
    let list = conn
        .execute(qapi_qmp::x_query_virtio {})
        .await
        .map_err(CmdError::from)?;

    let mut lines = Vec::new();
    for info in &list {
        lines.push(format!("{} [{}]", info.path, info.name));
    }
    Ok(lines.join("\n"))
}

pub async fn cmd_info_virtio_status(
    conn: &QmpConnection,
    args: &HashMap<String, ArgValue>,
) -> Result<String, CmdError> {
    let path = require_str(args, "path")?;

    let s = conn
        .execute(qapi_qmp::x_query_virtio_status { path: path.clone() })
        .await
        .map_err(CmdError::from)?;

    let mut out = String::new();
    writeln!(out, "{path}:").unwrap();
    writeln!(
        out,
        "  device_name:             {} {}",
        s.name,
        if s.vhost_dev.is_some() { "(vhost)" } else { "" }
    )
    .unwrap();
    writeln!(out, "  device_id:               {}", s.device_id).unwrap();
    writeln!(
        out,
        "  vhost_started:           {}",
        if s.vhost_started { "true" } else { "false" }
    )
    .unwrap();
    let bus = if s.bus_name.is_empty() {
        "(null)"
    } else {
        &s.bus_name
    };
    writeln!(out, "  bus_name:                {bus}").unwrap();
    writeln!(
        out,
        "  broken:                  {}",
        if s.broken { "true" } else { "false" }
    )
    .unwrap();
    writeln!(
        out,
        "  disabled:                {}",
        if s.disabled { "true" } else { "false" }
    )
    .unwrap();
    writeln!(
        out,
        "  disable_legacy_check:    {}",
        if s.disable_legacy_check {
            "true"
        } else {
            "false"
        }
    )
    .unwrap();
    writeln!(
        out,
        "  started:                 {}",
        if s.started { "true" } else { "false" }
    )
    .unwrap();
    writeln!(
        out,
        "  use_started:             {}",
        if s.use_started { "true" } else { "false" }
    )
    .unwrap();
    writeln!(
        out,
        "  start_on_kick:           {}",
        if s.start_on_kick { "true" } else { "false" }
    )
    .unwrap();
    writeln!(
        out,
        "  use_guest_notifier_mask: {}",
        if s.use_guest_notifier_mask {
            "true"
        } else {
            "false"
        }
    )
    .unwrap();
    writeln!(
        out,
        "  vm_running:              {}",
        if s.vm_running { "true" } else { "false" }
    )
    .unwrap();
    writeln!(out, "  num_vqs:                 {}", s.num_vqs).unwrap();
    writeln!(out, "  queue_sel:               {}", s.queue_sel).unwrap();
    writeln!(out, "  isr:                     {}", s.isr).unwrap();
    writeln!(out, "  endianness:              {}", s.device_endian).unwrap();
    writeln!(out, "  status:").unwrap();
    fmt_virtio_dump_status(&mut out, &s.status);
    writeln!(out, "  Guest features:").unwrap();
    fmt_virtio_dump_features(&mut out, &s.guest_features);
    writeln!(out, "  Host features:").unwrap();
    fmt_virtio_dump_features(&mut out, &s.host_features);
    writeln!(out, "  Backend features:").unwrap();
    fmt_virtio_dump_features(&mut out, &s.backend_features);

    if let Some(ref vhost) = s.vhost_dev {
        writeln!(out, "  VHost:").unwrap();
        writeln!(out, "    nvqs:           {}", vhost.nvqs).unwrap();
        writeln!(out, "    vq_index:       {}", vhost.vq_index).unwrap();
        writeln!(out, "    max_queues:     {}", vhost.max_queues).unwrap();
        writeln!(out, "    n_mem_sections: {}", vhost.n_mem_sections).unwrap();
        writeln!(out, "    n_tmp_sections: {}", vhost.n_tmp_sections).unwrap();
        writeln!(out, "    backend_cap:    {}", vhost.backend_cap).unwrap();
        writeln!(
            out,
            "    log_enabled:    {}",
            if vhost.log_enabled { "true" } else { "false" }
        )
        .unwrap();
        writeln!(out, "    log_size:       {}", vhost.log_size).unwrap();
        writeln!(out, "    Features:").unwrap();
        fmt_virtio_dump_features(&mut out, &vhost.features);
        writeln!(out, "    Acked features:").unwrap();
        fmt_virtio_dump_features(&mut out, &vhost.acked_features);
        writeln!(out, "    Backend features:").unwrap();
        fmt_virtio_dump_features(&mut out, &vhost.backend_features);
        writeln!(out, "    Protocol features:").unwrap();
        fmt_virtio_dump_protocols(&mut out, &vhost.protocol_features);
    }

    Ok(out)
}

pub async fn cmd_info_virtio_queue_element(
    conn: &QmpConnection,
    args: &HashMap<String, ArgValue>,
) -> Result<String, CmdError> {
    let path = require_str(args, "path")?;
    let queue = require_int(args, "queue")?;
    let index = match args.get("index") {
        Some(ArgValue::Int(n)) => Some(*n as u16),
        _ => None,
    };

    let e = conn
        .execute(qapi_qmp::x_query_virtio_queue_element {
            path: path.clone(),
            queue: queue as u16,
            index,
        })
        .await
        .map_err(CmdError::from)?;

    let mut out = String::new();
    writeln!(out, "{path}:").unwrap();
    writeln!(out, "  device_name: {}", e.name).unwrap();
    writeln!(out, "  index:   {}", e.index).unwrap();
    writeln!(out, "  desc:").unwrap();
    writeln!(out, "    descs:").unwrap();

    for (i, desc) in e.descs.iter().enumerate() {
        write!(out, "        addr 0x{:x} len {}", desc.addr, desc.len).unwrap();
        if !desc.flags.is_empty() {
            write!(out, " ({})", desc.flags.join(", ")).unwrap();
        }
        if i + 1 < e.descs.len() {
            writeln!(out, ",").unwrap();
        }
    }
    writeln!(out).unwrap();

    writeln!(out, "  avail:").unwrap();
    writeln!(out, "    flags: {}", e.avail.flags).unwrap();
    writeln!(out, "    idx:   {}", e.avail.idx).unwrap();
    writeln!(out, "    ring:  {}", e.avail.ring).unwrap();
    writeln!(out, "  used:").unwrap();
    writeln!(out, "    flags: {}", e.used.flags).unwrap();
    writeln!(out, "    idx:   {}", e.used.idx).unwrap();

    Ok(out)
}

pub async fn cmd_info_virtio_queue_status(
    conn: &QmpConnection,
    args: &HashMap<String, ArgValue>,
) -> Result<String, CmdError> {
    let path = require_str(args, "path")?;
    let queue = require_int(args, "queue")?;

    let s = conn
        .execute(qapi_qmp::x_query_virtio_queue_status {
            path: path.clone(),
            queue: queue as u16,
        })
        .await
        .map_err(CmdError::from)?;

    let mut out = String::new();
    writeln!(out, "{path}:").unwrap();
    writeln!(out, "  device_name:          {}", s.name).unwrap();
    writeln!(out, "  queue_index:          {}", s.queue_index).unwrap();
    writeln!(out, "  inuse:                {}", s.inuse).unwrap();
    writeln!(out, "  used_idx:             {}", s.used_idx).unwrap();
    writeln!(out, "  signalled_used:       {}", s.signalled_used).unwrap();
    writeln!(
        out,
        "  signalled_used_valid: {}",
        if s.signalled_used_valid {
            "true"
        } else {
            "false"
        }
    )
    .unwrap();
    if let Some(idx) = s.last_avail_idx {
        writeln!(out, "  last_avail_idx:       {idx}").unwrap();
    }
    if let Some(idx) = s.shadow_avail_idx {
        writeln!(out, "  shadow_avail_idx:     {idx}").unwrap();
    }
    writeln!(out, "  VRing:").unwrap();
    writeln!(out, "    num:          {}", s.vring_num).unwrap();
    writeln!(out, "    num_default:  {}", s.vring_num_default).unwrap();
    writeln!(out, "    align:        {}", s.vring_align).unwrap();
    writeln!(out, "    desc:         0x{:016x}", s.vring_desc).unwrap();
    writeln!(out, "    avail:        0x{:016x}", s.vring_avail).unwrap();
    writeln!(out, "    used:         0x{:016x}", s.vring_used).unwrap();

    Ok(out)
}

pub async fn cmd_info_virtio_vhost_queue_status(
    conn: &QmpConnection,
    args: &HashMap<String, ArgValue>,
) -> Result<String, CmdError> {
    let path = require_str(args, "path")?;
    let queue = require_int(args, "queue")?;

    let s = conn
        .execute(qapi_qmp::x_query_virtio_vhost_queue_status {
            path: path.clone(),
            queue: queue as u16,
        })
        .await
        .map_err(CmdError::from)?;

    let mut out = String::new();
    writeln!(out, "{path}:").unwrap();
    writeln!(out, "  device_name:          {} (vhost)", s.name).unwrap();
    writeln!(out, "  kick:                 {}", s.kick).unwrap();
    writeln!(out, "  call:                 {}", s.call).unwrap();
    writeln!(out, "  VRing:").unwrap();
    writeln!(out, "    num:         {}", s.num).unwrap();
    writeln!(out, "    desc:        0x{:016x}", s.desc).unwrap();
    writeln!(out, "    desc_phys:   0x{:016x}", s.desc_phys).unwrap();
    writeln!(out, "    desc_size:   {}", s.desc_size).unwrap();
    writeln!(out, "    avail:       0x{:016x}", s.avail).unwrap();
    writeln!(out, "    avail_phys:  0x{:016x}", s.avail_phys).unwrap();
    writeln!(out, "    avail_size:  {}", s.avail_size).unwrap();
    writeln!(out, "    used:        0x{:016x}", s.used).unwrap();
    writeln!(out, "    used_phys:   0x{:016x}", s.used_phys).unwrap();
    writeln!(out, "    used_size:   {}", s.used_size).unwrap();

    Ok(out)
}
