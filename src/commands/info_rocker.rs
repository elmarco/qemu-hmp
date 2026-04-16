// SPDX-License-Identifier: GPL-2.0-or-later

use std::collections::HashMap;
use std::fmt::Write;

use crate::args::ArgValue;
use crate::commands::{require_str, CmdError};
use crate::qmp::QmpConnection;

const VLAN_VID_MASK: u16 = 0x0fff;

pub async fn cmd_info_rocker(
    conn: &QmpConnection,
    args: &HashMap<String, ArgValue>,
) -> Result<String, CmdError> {
    let name = require_str(args, "name")?;

    let rocker = conn
        .execute(qapi_qmp::query_rocker { name: name.clone() })
        .await
        .map_err(CmdError::from)?;

    let mut out = String::new();
    writeln!(out, "name: {}", rocker.name).unwrap();
    writeln!(out, "id: 0x{:x}", rocker.id).unwrap();
    writeln!(out, "ports: {}", rocker.ports).unwrap();
    Ok(out)
}

pub async fn cmd_info_rocker_ports(
    conn: &QmpConnection,
    args: &HashMap<String, ArgValue>,
) -> Result<String, CmdError> {
    let name = require_str(args, "name")?;

    let ports = conn
        .execute(qapi_qmp::query_rocker_ports { name: name.clone() })
        .await
        .map_err(CmdError::from)?;

    let mut out = String::new();
    writeln!(out, "            ena/    speed/ auto").unwrap();
    writeln!(out, "      port  link    duplex neg?").unwrap();

    for port in &ports {
        let ena_link = if !port.enabled {
            "!ena"
        } else if port.link_up {
            "up"
        } else {
            "down"
        };
        let speed = if port.speed == 10000 { "10G" } else { "??" };
        let duplex = match port.duplex {
            qapi_qmp::RockerPortDuplex::full => "FD",
            qapi_qmp::RockerPortDuplex::half => "HD",
        };
        let autoneg = match port.autoneg {
            qapi_qmp::RockerPortAutoneg::on => "Yes",
            qapi_qmp::RockerPortAutoneg::off => "No",
        };
        writeln!(
            out,
            "{:>10}  {:<4}   {:<3}  {:>2}  {}",
            port.name, ena_link, speed, duplex, autoneg
        )
        .unwrap();
    }

    Ok(out)
}

pub async fn cmd_info_rocker_of_dpa_flows(
    conn: &QmpConnection,
    args: &HashMap<String, ArgValue>,
) -> Result<String, CmdError> {
    let name = require_str(args, "name")?;
    let tbl_id = match args.get("tbl_id") {
        Some(ArgValue::Int(n)) => Some(*n as u32),
        _ => None,
    };

    let flows = conn
        .execute(qapi_qmp::query_rocker_of_dpa_flows {
            name: name.clone(),
            tbl_id,
        })
        .await
        .map_err(CmdError::from)?;

    let mut out = String::new();
    writeln!(out, "prio tbl hits key(mask) --> actions").unwrap();

    for flow in &flows {
        let key = &flow.key;
        let mask = &flow.mask;
        let action = &flow.action;

        if flow.hits != 0 {
            write!(
                out,
                "{:<4} {:<3} {:<4}",
                key.priority, key.tbl_id, flow.hits
            )
            .unwrap();
        } else {
            write!(out, "{:<4} {:<3}     ", key.priority, key.tbl_id).unwrap();
        }

        if let Some(in_pport) = key.in_pport {
            write!(out, " pport {}", in_pport).unwrap();
            if let Some(m) = mask.in_pport {
                write!(out, "(0x{:x})", m).unwrap();
            }
        }

        if let Some(vlan_id) = key.vlan_id {
            write!(out, " vlan {}", vlan_id & VLAN_VID_MASK).unwrap();
            if let Some(m) = mask.vlan_id {
                write!(out, "(0x{:x})", m).unwrap();
            }
        }

        if let Some(tunnel_id) = key.tunnel_id {
            write!(out, " tunnel {}", tunnel_id).unwrap();
            if let Some(m) = mask.tunnel_id {
                write!(out, "(0x{:x})", m).unwrap();
            }
        }

        if let Some(eth_type) = key.eth_type {
            match eth_type {
                0x0806 => write!(out, " ARP").unwrap(),
                0x0800 => write!(out, " IP").unwrap(),
                0x86dd => write!(out, " IPv6").unwrap(),
                0x8809 => write!(out, " LACP").unwrap(),
                0x88cc => write!(out, " LLDP").unwrap(),
                _ => write!(out, " eth type 0x{:04x}", eth_type).unwrap(),
            }
        }

        if let Some(ref eth_src) = key.eth_src {
            if eth_src == "01:00:00:00:00:00"
                && mask.eth_src.as_deref() == Some("01:00:00:00:00:00")
            {
                write!(out, " src <any mcast/bcast>").unwrap();
            } else if eth_src == "00:00:00:00:00:00"
                && mask.eth_src.as_deref() == Some("01:00:00:00:00:00")
            {
                write!(out, " src <any ucast>").unwrap();
            } else {
                write!(out, " src {}", eth_src).unwrap();
                if let Some(ref m) = mask.eth_src {
                    write!(out, "({})", m).unwrap();
                }
            }
        }

        if let Some(ref eth_dst) = key.eth_dst {
            if eth_dst == "01:00:00:00:00:00"
                && mask.eth_dst.as_deref() == Some("01:00:00:00:00:00")
            {
                write!(out, " dst <any mcast/bcast>").unwrap();
            } else if eth_dst == "00:00:00:00:00:00"
                && mask.eth_dst.as_deref() == Some("01:00:00:00:00:00")
            {
                write!(out, " dst <any ucast>").unwrap();
            } else {
                write!(out, " dst {}", eth_dst).unwrap();
                if let Some(ref m) = mask.eth_dst {
                    write!(out, "({})", m).unwrap();
                }
            }
        }

        if let Some(ip_proto) = key.ip_proto {
            write!(out, " proto {}", ip_proto).unwrap();
            if let Some(m) = mask.ip_proto {
                write!(out, "(0x{:x})", m).unwrap();
            }
        }

        if let Some(ip_tos) = key.ip_tos {
            write!(out, " TOS {}", ip_tos).unwrap();
            if let Some(m) = mask.ip_tos {
                write!(out, "(0x{:x})", m).unwrap();
            }
        }

        if let Some(ref ip_dst) = key.ip_dst {
            write!(out, " dst {}", ip_dst).unwrap();
        }

        if action.goto_tbl.is_some() || action.group_id.is_some() || action.new_vlan_id.is_some() {
            write!(out, " -->").unwrap();
        }

        if let Some(new_vlan_id) = action.new_vlan_id {
            write!(out, " apply new vlan {}", u16::from_be(new_vlan_id)).unwrap();
        }

        if let Some(group_id) = action.group_id {
            write!(out, " write group 0x{:08x}", group_id).unwrap();
        }

        if let Some(goto_tbl) = action.goto_tbl {
            write!(out, " goto tbl {}", goto_tbl).unwrap();
        }

        writeln!(out).unwrap();
    }

    Ok(out)
}

pub async fn cmd_info_rocker_of_dpa_groups(
    conn: &QmpConnection,
    args: &HashMap<String, ArgValue>,
) -> Result<String, CmdError> {
    let name = require_str(args, "name")?;
    let type_ = match args.get("type") {
        Some(ArgValue::Int(n)) => Some(*n as u8),
        _ => None,
    };

    let groups = conn
        .execute(qapi_qmp::query_rocker_of_dpa_groups {
            name: name.clone(),
            type_,
        })
        .await
        .map_err(CmdError::from)?;

    let mut out = String::new();
    writeln!(out, "id (decode) --> buckets").unwrap();

    for group in &groups {
        write!(out, "0x{:08x}", group.id).unwrap();

        let type_name = match group.type_ {
            0 => "L2 interface",
            1 => "L2 rewrite",
            2 => "L3 unicast",
            3 => "L2 multicast",
            4 => "L2 flood",
            5 => "L3 interface",
            6 => "L3 multicast",
            7 => "L3 ECMP",
            8 => "L2 overlay",
            _ => "unknown",
        };

        write!(out, " (type {}", type_name).unwrap();

        if let Some(vlan_id) = group.vlan_id {
            write!(out, " vlan {}", vlan_id).unwrap();
        }

        if let Some(pport) = group.pport {
            write!(out, " pport {}", pport).unwrap();
        }

        if let Some(index) = group.index {
            write!(out, " index {}", index).unwrap();
        }

        write!(out, ") -->").unwrap();

        let mut set = false;

        if let Some(set_vlan_id) = group.set_vlan_id.filter(|&v| v != 0) {
            set = true;
            write!(out, " set vlan {}", set_vlan_id & VLAN_VID_MASK).unwrap();
        }

        if let Some(ref src) = group.set_eth_src {
            if !set {
                set = true;
                write!(out, " set").unwrap();
            }
            write!(out, " src {}", src).unwrap();
        }

        if let Some(ref dst) = group.set_eth_dst {
            if !set {
                write!(out, " set").unwrap();
            }
            write!(out, " dst {}", dst).unwrap();
        }

        if group.ttl_check.filter(|&v| v != 0).is_some() {
            write!(out, " check TTL").unwrap();
        }

        if let Some(group_id) = group.group_id.filter(|&v| v != 0) {
            write!(out, " group id 0x{:08x}", group_id).unwrap();
        }

        if group.pop_vlan.filter(|&v| v != 0).is_some() {
            write!(out, " pop vlan").unwrap();
        }

        if let Some(out_pport) = group.out_pport {
            write!(out, " out pport {}", out_pport).unwrap();
        }

        if let Some(ref group_ids) = group.group_ids {
            write!(out, " groups [").unwrap();
            for (i, id) in group_ids.iter().enumerate() {
                if i > 0 {
                    write!(out, ",").unwrap();
                }
                write!(out, "0x{:08x}", id).unwrap();
            }
            write!(out, "]").unwrap();
        }

        writeln!(out).unwrap();
    }

    Ok(out)
}
