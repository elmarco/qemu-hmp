// SPDX-License-Identifier: GPL-2.0-or-later

use std::collections::HashMap;
use std::fmt::Write;

use crate::args::ArgValue;
use crate::commands::CmdError;
use crate::qmp::QmpConnection;

/// PCI_BAR_UNMAPPED in QEMU is ~(pcibus_t)0 which serializes as -1 in JSON.
const PCI_BAR_UNMAPPED: i64 = -1;

fn format_pci_device(out: &mut String, dev: &qapi_qmp::PciDeviceInfo) {
    write!(out, "  Bus {:2}, ", dev.bus).unwrap();
    writeln!(out, "device {:3}, function {}:", dev.slot, dev.function).unwrap();
    write!(out, "    ").unwrap();

    if let Some(ref desc) = dev.class_info.desc {
        write!(out, "{desc}").unwrap();
    } else {
        write!(out, "Class {:04}", dev.class_info.class).unwrap();
    }

    writeln!(
        out,
        ": PCI device {:04x}:{:04x}",
        dev.id.vendor, dev.id.device
    )
    .unwrap();

    if let (Some(sub_vendor), Some(sub)) = (dev.id.subsystem_vendor, dev.id.subsystem) {
        writeln!(out, "      PCI subsystem {:04x}:{:04x}", sub_vendor, sub).unwrap();
    }

    if let Some(irq) = dev.irq {
        let pin = (b'A' + dev.irq_pin as u8 - 1) as char;
        writeln!(out, "      IRQ {irq}, pin {pin}").unwrap();
    }

    if let Some(ref bridge) = dev.pci_bridge {
        writeln!(out, "      BUS {}.", bridge.bus.number).unwrap();
        writeln!(out, "      secondary bus {}.", bridge.bus.secondary).unwrap();
        writeln!(out, "      subordinate bus {}.", bridge.bus.subordinate).unwrap();

        writeln!(
            out,
            "      IO range [0x{:04x}, 0x{:04x}]",
            bridge.bus.io_range.base, bridge.bus.io_range.limit
        )
        .unwrap();

        writeln!(
            out,
            "      memory range [0x{:08x}, 0x{:08x}]",
            bridge.bus.memory_range.base, bridge.bus.memory_range.limit
        )
        .unwrap();

        writeln!(
            out,
            "      prefetchable memory range [0x{:08x}, 0x{:08x}]",
            bridge.bus.prefetchable_range.base, bridge.bus.prefetchable_range.limit
        )
        .unwrap();
    }

    for region in &dev.regions {
        let addr = region.address;
        let size = region.size;

        write!(out, "      BAR{}: ", region.bar).unwrap();

        if region.type_ == "io" {
            if addr != PCI_BAR_UNMAPPED {
                let end = (addr as u64).wrapping_add(size as u64).wrapping_sub(1);
                writeln!(out, "I/O at 0x{:04x} [0x{:04x}]", addr, end).unwrap();
            } else {
                writeln!(out, "I/O (not mapped)").unwrap();
            }
        } else if addr != PCI_BAR_UNMAPPED {
            let bits = if region.mem_type_64.unwrap_or(false) {
                64
            } else {
                32
            };
            let pf = if region.prefetch.unwrap_or(false) {
                " prefetchable"
            } else {
                ""
            };
            let end = (addr as u64).wrapping_add(size as u64).wrapping_sub(1);
            writeln!(
                out,
                "{bits} bit{pf} memory at 0x{:08x} [0x{:08x}]",
                addr, end
            )
            .unwrap();
        } else {
            let bits = if region.mem_type_64.unwrap_or(false) {
                64
            } else {
                32
            };
            let pf = if region.prefetch.unwrap_or(false) {
                " prefetchable"
            } else {
                ""
            };
            writeln!(out, "{bits} bit{pf} memory (not mapped)").unwrap();
        }
    }

    writeln!(out, "      id \"{}\"", dev.qdev_id).unwrap();

    if let Some(ref bridge) = dev.pci_bridge {
        if let Some(ref devices) = bridge.devices {
            for cdev in devices {
                format_pci_device(out, cdev);
            }
        }
    }
}

pub async fn cmd_info_pci(
    conn: &QmpConnection,
    _args: &HashMap<String, ArgValue>,
) -> Result<String, CmdError> {
    let info_list = conn
        .execute(qapi_qmp::query_pci {})
        .await
        .map_err(CmdError::from)?;

    let mut out = String::new();
    for info in &info_list {
        for dev in &info.devices {
            format_pci_device(&mut out, dev);
        }
    }

    Ok(out)
}
