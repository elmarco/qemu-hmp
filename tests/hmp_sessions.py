# SPDX-License-Identifier: GPL-2.0-or-later
"""
QEMU session configurations for compare_hmp.py.

Each session is a dict with:
  name           - display name (used in output headers and summary)
  session_id     - unique id (used for socket file naming)
  qemu_args      - extra QEMU command-line arguments
  commands       - list of commands (str or (str, skip_reason) tuples)
  stateful_tests - list of {"name": ..., "commands": [...]} dicts
"""

import os
import shutil
import subprocess

from hmp_commands import COMMANDS, STATEFUL_TESTS, DESTABILIZING_TESTS


BALLOON_SESSION = {
    "name": "balloon",
    "session_id": "balloon",
    "qemu_args": [
        "-M", "pc",
        "-m", "256",
        "-name", "hmp-balloon-test",
        "-device", "virtio-balloon-pci,id=balloon0",
    ],
    "commands": [
        "info balloon",
        "info virtio",
    ],
    "stateful_tests": [
        {
            "name": "balloon set/query",
            "commands": [
                "balloon 128",
                "info balloon",
                "balloon 256",
                "info balloon",
            ],
        },
        {
            "name": "info virtio-status",
            "commands": [
                "info virtio-status /machine/peripheral/balloon0/virtio-backend",
            ],
        },
        {
            "name": "info virtio-queue-status",
            "commands": [
                "info virtio-queue-status /machine/peripheral/balloon0/virtio-backend 0",
            ],
        },
    ],
}

NETWORK_SESSION = {
    "name": "network",
    "session_id": "net",
    "qemu_args": [
        "-M", "pc",
        "-name", "hmp-net-test",
        "-netdev", "user,id=net0",
        "-device", "e1000,netdev=net0,id=nic0",
    ],
    "commands": [
        "info network",
        "info usernet",
    ],
    "stateful_tests": [
        {
            "name": "set_link off/on",
            "commands": [
                "set_link net0 off",
                "set_link net0 on",
            ],
        },
        {
            "name": "set_link invalid device",
            "commands": [
                "set_link nosuchdev off",
            ],
        },
        {
            "name": "hostfwd_add/remove via net0",
            "commands": [
                "hostfwd_add net0 tcp::19080-:80",
                "hostfwd_add net0 udp::19053-:53",
                "hostfwd_remove net0 tcp::19080",
                "hostfwd_remove net0 udp::19053",
            ],
        },
    ],
}

USB_SESSION = {
    "name": "usb",
    "session_id": "usb",
    "qemu_args": [
        "-M", "pc",
        "-name", "hmp-usb-test",
        "-usb",
        "-device", "usb-tablet,id=tablet0",
    ],
    "commands": [
        "info usb",
    ],
    "stateful_tests": [
        {
            "name": "mouse_set with USB tablet",
            "commands": [
                "info mice",
                "mouse_set 0",
                "info mice",
            ],
        },
    ],
}

SMP_SESSION = {
    "name": "smp",
    "session_id": "smp",
    "qemu_args": [
        "-M", "pc",
        "-name", "hmp-smp-test",
        "-smp", "4,sockets=2,cores=2",
    ],
    "commands": [
        "info cpus",
        "info hotpluggable-cpus",
    ],
    "stateful_tests": [
        {
            "name": "cpu select valid indices",
            "commands": [
                "cpu 0",
                "info cpus",
                "cpu 2",
                "info cpus",
                "cpu 3",
                "info cpus",
            ],
        },
        {
            "name": "cpu select invalid index",
            "commands": [
                "cpu 999",
            ],
        },
    ],
}

NUMA_SESSION = {
    "name": "numa",
    "session_id": "numa",
    "qemu_args": [
        "-M", "pc",
        "-name", "hmp-numa-test",
        "-smp", "4",
        "-m", "256",
        "-object", "memory-backend-ram,id=mem0,size=128M",
        "-object", "memory-backend-ram,id=mem1,size=128M",
        "-numa", "node,memdev=mem0,cpus=0-1",
        "-numa", "node,memdev=mem1,cpus=2-3",
    ],
    "commands": [
        "info numa",
        "info cpus",
        "info memdev",
        "info hotpluggable-cpus",
    ],
    "stateful_tests": [],
}

IOTHREAD_SESSION = {
    "name": "iothread",
    "session_id": "ioth",
    "qemu_args": [
        "-M", "pc",
        "-name", "hmp-iothread-test",
        "-object", "iothread,id=iothread0",
        "-object", "iothread,id=iothread1",
    ],
    "commands": [
        "info iothreads",
    ],
    "stateful_tests": [],
}

MEMORY_HOTPLUG_SESSION = {
    "name": "memory-hotplug",
    "session_id": "memhp",
    "qemu_args": [
        "-M", "pc",
        "-name", "hmp-memhp-test",
        "-m", "256,maxmem=1G,slots=2",
        "-object", "memory-backend-ram,id=mem-dimm0,size=128M",
        "-device", "pc-dimm,id=dimm0,memdev=mem-dimm0",
    ],
    "commands": [
        "info memory-devices",
        "info memory_size_summary",
        "info memdev",
    ],
    "stateful_tests": [],
}

SPICE_SESSION = {
    "name": "spice",
    "session_id": "spice",
    "qemu_args": [
        "-M", "pc",
        "-name", "hmp-spice-test",
        "-object", "secret,id=spicepw,data=initial",
        "-spice", "port=5930,password-secret=spicepw",
    ],
    "commands": [
        "info spice",
    ],
    "stateful_tests": [
        {
            "name": "set_password spice",
            "commands": [
                "set_password spice testpw123",
                "set_password spice newpw keep",
                "set_password spice pw disconnect",
            ],
        },
        {
            "name": "expire_password spice",
            "commands": [
                "expire_password spice now",
                "expire_password spice never",
                "expire_password spice +30",
            ],
        },
        {
            "name": "client_migrate_info spice",
            "commands": [
                "client_migrate_info spice virt42.lab.kraxel.org 1234",
            ],
        },
    ],
}


PCIE_AER_SESSION = {
    "name": "pcie-aer",
    "session_id": "pcie",
    "qemu_args": [
        "-M", "q35",
        "-name", "hmp-pcie-aer-test",
        "-device", "e1000e,id=e1000e0",
    ],
    "commands": [],
    "stateful_tests": [
        {
            "name": "pcie_aer_inject_error named errors",
            "commands": [
                "pcie_aer_inject_error e1000e0 RCVR",
                "pcie_aer_inject_error e1000e0 DLP",
            ],
        },
        {
            "name": "pcie_aer_inject_error numeric correctable",
            "commands": [
                "pcie_aer_inject_error -c e1000e0 0x0001",
            ],
        },
    ],
}


def build_main_session(tmpdir, qemu_bin):
    """Build the main session config (depends on temp file paths)."""
    cd_image = os.path.join(tmpdir, "test-cd.raw")
    with open(cd_image, "wb") as f:
        f.truncate(1024 * 1024)

    screendump_ppm = os.path.join(tmpdir, "screen.ppm")
    screendump_png = os.path.join(tmpdir, "screen.png")
    log_file = os.path.join(tmpdir, "qemu-test.log")
    vnc_sock = os.path.join(tmpdir, "main-vnc.sock")

    snap_disk = os.path.join(tmpdir, "snap.qcow2")
    qemu_img = os.path.join(os.path.dirname(qemu_bin), "qemu-img")
    if not os.path.isfile(qemu_img):
        qemu_img = shutil.which("qemu-img")
    if qemu_img:
        subprocess.run(
            [qemu_img, "create", "-f", "qcow2", snap_disk, "10M"],
            check=True,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
        )
    has_snap = os.path.isfile(snap_disk)

    overlay_disk = os.path.join(tmpdir, "overlay.qcow2")
    base_disk = os.path.join(tmpdir, "base.raw")
    has_overlay = False
    if qemu_img:
        with open(base_disk, "wb") as f:
            f.truncate(10 * 1024 * 1024)
        subprocess.run(
            [qemu_img, "create", "-f", "qcow2",
             "-b", base_disk, "-F", "raw", overlay_disk, "10M"],
            check=True,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
        )
        has_overlay = os.path.isfile(overlay_disk)

    qemu_args = [
        "-M", "pc",
        "-name", "hmp-compare-test",
        "-vnc", f"unix:{vnc_sock},password=on",
        "-drive", "if=ide,index=2,media=cdrom,id=cd0",
        "-chardev", "ringbuf,id=ringbuf0",
    ]
    if has_snap:
        qemu_args += [
            "-drive",
            f"file={snap_disk},format=qcow2,if=ide,id=snap0",
        ]
    if has_overlay:
        qemu_args += [
            "-drive",
            f"file={overlay_disk},format=qcow2,if=none,id=overlay0",
        ]

    dump_elf = os.path.join(tmpdir, "dump.elf")
    with open(dump_elf, "wb"):
        pass

    stateful = []
    if has_snap:
        stateful.append({
            "name": "savevm/loadvm/delvm",
            "commands": [
                "savevm test-snap",
                "loadvm test-snap",
                "delvm test-snap",
            ],
        })

    stateful += list(STATEFUL_TESTS)

    stateful += [
        {
            "name": "change vnc password",
            "commands": ["change vnc password testpw123"],
        },
        {
            "name": "change cd-rom medium",
            "commands": [
                f"change cd0 {cd_image}",
                "eject cd0",
            ],
        },
        {
            "name": "screendump",
            "commands": [
                f"screendump {screendump_ppm}",
                f"screendump {screendump_png} -f png",
            ],
        },
        {
            "name": "trace-event toggle",
            "commands": [
                "trace-event monitor_qmp_respond on",
                "trace-event monitor_qmp_respond off",
            ],
        },
        # trace-file on/off: x-trace-file QMP command not available in all QEMU builds
        {
            "name": "log",
            "commands": ["log in", "log none"],
        },
        {
            "name": "logfile",
            "commands": [f"logfile {log_file}"],
        },
        {
            "name": "dump-guest-memory errors",
            "commands": [
                "dump-guest-memory -z -l /tmp/bad.elf",
                "dump-guest-memory -z -s /tmp/bad.elf",
                "dump-guest-memory -w -l /tmp/bad.elf",
            ],
        },
        {
            "name": "dump-guest-memory",
            "commands": [
                f"dump-guest-memory {dump_elf}",
            ],
        },
        {
            "name": "memsave",
            "commands": [
                f"memsave 0 64 memsave.bin",
            ],
        },
        {
            "name": "pmemsave",
            "commands": [
                f"pmemsave 0 64 pmemsave.bin",
            ],
        },
    ]

    if has_snap:
        stateful += [
            {
                "name": "snapshot_blkdev",
                "commands": ["snapshot_blkdev snap0"],
            },
            {
                "name": "snapshot_blkdev_internal lifecycle",
                "commands": [
                    "snapshot_blkdev_internal snap0 test-int-snap",
                    "snapshot_delete_blkdev_internal snap0 test-int-snap",
                ],
            },
            {
                "name": "snapshot_blkdev_internal error",
                "commands": [
                    "snapshot_blkdev_internal nosuchdev test-snap",
                ],
            },
            {
                "name": "snapshot_delete_blkdev_internal error",
                "commands": [
                    "snapshot_delete_blkdev_internal nosuchdev test-snap",
                ],
            },
            {
                "name": "drive_backup error",
                "commands": [
                    "drive_backup nosuchdev /tmp/backup.qcow2",
                ],
            },
            {
                "name": "drive_mirror error",
                "commands": [
                    "drive_mirror nosuchdev /tmp/mirror.qcow2",
                ],
            },
        ]

    if has_overlay:
        stateful += [
            {
                "name": "commit overlay",
                "commands": ["commit overlay0"],
            },
        ]

    stateful += list(DESTABILIZING_TESTS)

    return {
        "name": "main",
        "session_id": "main",
        "qemu_args": qemu_args,
        "commands": COMMANDS,
        "stateful_tests": stateful,
    }
