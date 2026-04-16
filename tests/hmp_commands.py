# SPDX-License-Identifier: GPL-2.0-or-later
"""
HMP command definitions for compare_hmp.py.

Each entry in COMMANDS is either a command string or a
(command_string, skip_reason) tuple.  Skipped commands are listed
in the output but not executed.

Each entry in STATEFUL_TESTS is a named group of commands that run
in order.  The entire sequence is run through HMP first, then through
qemu-hmp, so both sides see the same state transitions.
"""

COMMANDS = [
    # ── info subcommands ─────────────────────────────────────────
    "info version",
    "info status",
    "info kvm",
    "info cpus",
    "info chardev",
    "info block",
    "info blockstats",
    "info name",
    "info uuid",
    "info migrate",
    "info vnc",
    "info mice",
    "info replay",
    "info cryptodev",
    "info dirty_rate",
    "info dump",
    "info hotpluggable-cpus",
    "info ramblock",
    "info memory_size_summary",
    "info vm-generation-id",
    "info iothreads",
    "info memory-devices",
    "info memdev",
    "info tpm",
    "info trace-events",
    "info roms",
    "info migrate_parameters",
    "info migrate_capabilities",
    "info snapshots",
    "info spice",
    "info usbhost",
    "info usernet",
    "info usb",
    "info numa",
    "info lapic",
    "info registers",
    "info network",
    "info mem",
    "info tlb",
    "info mtree",
    "info sync-profile",
    "info accelerators",
    "info irq",
    "info block-jobs",
    "info pci",
    "info pic",
    "info qom-tree",
    "info qdm",
    "info qtree",
    "info sev",
    "info sgx",
    "info vcpu_dirty_limit",
    "info virtio",

    # ── main commands ────────────────────────────────────────────
    "device_del none0",
    "xp /8xw 0xb8000",
    "xp /4xb 0xb8000",
    "xp /4xg 0xb8000",
    "xp /16cb 0xb8000",
    "xp /4dw 0xb8000",
    "xp /4ow 0xb8000",
    "gva2gpa 0xb8000",
    "print /x 255",
    "print /o 255",
    "print /d 255",
    "print /u 255",
    "print /c 65",
    "print /x 0",
    "print /d -1",
    "print /x 1 + 2",
    "print /x (0x10 + 3) * 2",
    "print /d 'A'",
    "p /x 16",
    "p /d 42",
    "o/b 0x80 0",
    "o 0x80 0",
    "sendkey ret",
    "sendkey ctrl-alt-f1",
    "sendkey 0x1c",
    "qom-list",
    "qom-list /machine",
    "qom-get /machine type",
    "qom-list /nosuchpath",
    "qom-get /machine nosuchprop",

    # ── skipped: destructive ─────────────────────────────────────
    ("help", "handled locally; output intentionally differs"),
    ("quit", "destructive"),
    ("system_reset", "destructive"),
    ("system_powerdown", "destructive"),
    ("drive_del drive0", "destructive"),
    ("migrate -d tcp:localhost:12345", "starts migration"),

    # ── skipped: non-deterministic output ────────────────────────
    ("info accel", "dynamic statistics differ between queries"),
    ("info jit", "dynamic statistics differ between queries"),
    ("gpa2hva 0", "HVA varies per run due to ASLR"),
    ("gpa2hpa 0", "HPA varies and requires /proc/self/pagemap access"),
    ("x /4xw 0", "firmware may update low memory between sequential reads"),
    ("sum 0 4096", "firmware may update low memory between sequential reads"),
    ("i/b 0x61", "port 0x61 refresh bit toggles between reads"),
    ("i/w 0x61", "port 0x61 refresh bit toggles between reads"),
    ("i 0x61", "port 0x61 refresh bit toggles between reads"),

    # ── skipped: requires specific compile-time config ───────────
    ("trace-file", "requires CONFIG_TRACE_SIMPLE"),
    ("dumpdtb /tmp/test.dtb", "requires CONFIG_FDT"),
    ("info via", "requires CONFIG_MOS6522"),
    ("xen-event-inject 1", "requires CONFIG_XEN_EMU"),
    ("xen-event-list", "requires CONFIG_XEN_EMU"),

    # ── skipped: requires specific hardware / guest arch ─────────
    ("dump-skeys /tmp/skeys", "requires s390x guest"),
    ("migration_mode 1", "requires s390x guest"),
    ("info cmma 0", "requires s390x guest"),
    ("info skeys 0", "requires s390x guest"),
    ("info rocker sw1", "requires rocker device"),
    ("info rocker-ports sw1", "requires rocker device"),
    ("info rocker-of-dpa-flows sw1", "requires rocker device"),
    ("info rocker-of-dpa-groups sw1", "requires rocker device"),

    # ── skipped: requires specific runtime state ─────────────────
    ("x_colo_lost_heartbeat", "requires COLO replication"),
    ("migrate_start_postcopy", "requires active migration"),
    ("client_migrate_info spice virt42.lab.kraxel.org 1234", "requires SPICE display"),
    ("info firmware-log", "requires firmware log buffer"),
    ("info virtio-queue-element /foo 0", "requires virtio device"),
    ("info virtio-vhost-queue-status /foo 0", "requires vhost device"),

    # ── skipped: error path already covered in stateful tests ────
    ("block_resize drive0 1G", "drive0 not in main session; tested with snap0/nosuchdev"),
    ("block_stream drive0", "drive0 not in main session; tested with nosuchdev"),
    ("chardev-remove foo", "tested in stateful tests with nosuchchardev"),
    ("chardev-send-break foo", "tested in stateful tests with nosuchchardev"),
    ("netdev_del net0", "net0 not in main session; tested in stateful tests"),

    # ── skipped: known bugs / limitations ────────────────────────
    ("info qtree -b", "brief output has duplicate lines that confuse HMP reader"),
    ("info stats vm", "StatsFilter::vm serialization bug in qapi-rs"),
]


STATEFUL_TESTS = [
    {
        "name": "exit_preconfig already running",
        "commands": [
            "exit_preconfig",
        ],
    },
    {
        "name": "migrate_incoming no -incoming defer",
        "commands": [
            "migrate_incoming tcp:0:12345",
        ],
    },
    {
        "name": "migrate_pause no active postcopy",
        "commands": [
            "migrate_pause",
        ],
    },
    {
        "name": "migrate_recover no paused postcopy",
        "commands": [
            "migrate_recover tcp:0:0",
        ],
    },
    {
        "name": "announce_self",
        "commands": [
            "announce_self",
        ],
    },
    {
        "name": "migrate_set_parameter",
        "commands": [
            "migrate_set_parameter downtime-limit 500",
            "migrate_set_parameter cpu-throttle-initial 30",
            "migrate_set_parameter badparam 123",
        ],
    },
    {
        "name": "migrate_set_capability toggle",
        "commands": [
            "migrate_set_capability events on",
            "migrate_set_capability events off",
            "migrate_set_capability badcap on",
        ],
    },
    {
        "name": "migrate_continue invalid state",
        "commands": [
            "migrate_continue badstate",
        ],
    },
    {
        "name": "boot_set",
        "commands": [
            "boot_set c",
        ],
    },
    # device_add/del: skipped because device_del for PCI devices requires
    # guest ACPI cooperation (hot-unplug), which doesn't work without a
    # running guest OS. The device is never removed, causing duplicate-ID
    # errors on the second run and cascading failures in later tests.
    # Both commands are tested individually via the non-stateful command list.
    {
        "name": "object add/del lifecycle",
        "commands": [
            "object_add rng-random,id=test-obj0",
            "object_del test-obj0",
        ],
    },
    {
        "name": "drive_add -n / drive_del lifecycle",
        "commands": [
            "drive_add -n 0 driver=null-co,node-name=test-drv-add",
            "drive_del test-drv-add",
        ],
    },
    {
        "name": "netdev_add / netdev_del lifecycle",
        "commands": [
            "netdev_add user,id=test-net0",
            "netdev_del test-net0",
        ],
    },
    {
        "name": "hostfwd_add/remove with user netdev",
        "commands": [
            "netdev_add user,id=test-hfwd0",
            "hostfwd_add test-hfwd0 tcp::18080-:80",
            "hostfwd_add test-hfwd0 udp::15353-:53",
            "hostfwd_remove test-hfwd0 tcp::18080",
            "hostfwd_remove test-hfwd0 tcp::18080",
            "hostfwd_add nosuchdev tcp::1234-:22",
            "hostfwd_remove nosuchdev tcp::1234",
            "netdev_del test-hfwd0",
        ],
    },
    {
        "name": "cpu select valid/invalid",
        "commands": [
            "cpu 0",
            "cpu 999",
        ],
    },
    {
        "name": "ringbuf write/read cycle",
        "commands": [
            "ringbuf_write ringbuf0 hello",
            "ringbuf_read ringbuf0 5",
        ],
    },
    {
        "name": "mouse set/info cycle",
        "commands": [
            "info mice",
            "mouse_set 0",
            "info mice",
            "mouse_set 999",
        ],
    },
    {
        "name": "mouse button state cycle",
        "commands": [
            "info mice",
            "mouse_button 1",
            "mouse_button 3",
            "mouse_button 0",
            "mouse_move 10 20",
            "mouse_move 0 0 1",
            "info mice",
        ],
    },
    {
        "name": "qom-set/get",
        "commands": [
            "qom-set /machine vmname test-qom-set",
            "qom-get /machine vmname",
        ],
    },
    {
        "name": "sync-profile on/off/reset",
        "commands": [
            "sync-profile",
            "sync-profile on",
            "sync-profile",
            "sync-profile off",
            "sync-profile",
            "sync-profile reset",
        ],
    },
    {
        "name": "gdbserver start/stop",
        "commands": [
            "gdbserver tcp::9876",
            "gdbserver none",
        ],
    },
    {
        "name": "client_migrate_info",
        "commands": [
            "client_migrate_info spice virt42.lab.kraxel.org",
        ],
    },
    {
        "name": "pcie_aer_inject_error not found",
        "commands": [
            "pcie_aer_inject_error nosuchdev DLP",
        ],
    },
    {
        "name": "watchdog_action",
        "commands": [
            "watchdog_action reset",
            "watchdog_action pause",
            "watchdog_action none",
            "watchdog_action inject-nmi",
            "watchdog_action badaction",
        ],
    },
    {
        "name": "mce injection",
        "commands": [
            "mce 0 0 0x8000000000000000 0 0 0",
        ],
    },
    {
        "name": "closefd not found",
        "commands": [
            "closefd nosuchfd",
        ],
    },
    {
        "name": "getfd without fd",
        "commands": [
            "getfd testfd",
        ],
    },
    {
        "name": "block_set_io_throttle not found",
        "commands": [
            "block_set_io_throttle nosuchdev 0 0 0 0 0 0",
        ],
    },
    {
        "name": "nbd_server lifecycle",
        "commands": [
            "nbd_server_stop",
            "nbd_server_start localhost:10809",
            "nbd_server_start localhost:10809",
            "nbd_server_stop",
        ],
    },
    {
        "name": "nbd_server add/remove",
        "commands": [
            "nbd_server_start localhost:10809",
            "nbd_server_add snap0",
            "nbd_server_add -w snap0 snap0-alias",
            "nbd_server_remove snap0-alias",
            "nbd_server_remove snap0",
            "nbd_server_remove nosuchexport",
            "nbd_server_stop",
        ],
    },
    {
        "name": "nbd_server_start -w without -a",
        "commands": [
            "nbd_server_start -w localhost:10809",
        ],
    },
    {
        "name": "set_password vnc",
        "commands": [
            "set_password vnc testpw123",
            "set_password vnc newpw keep",
            "set_password vnc pw disconnect",
        ],
    },
    {
        "name": "set_password spice not enabled",
        "commands": [
            "set_password spice testpw",
        ],
    },
    {
        "name": "expire_password vnc",
        "commands": [
            "expire_password vnc now",
            "expire_password vnc never",
            "expire_password vnc +30",
        ],
    },
    {
        "name": "expire_password spice not enabled",
        "commands": [
            "expire_password spice now",
        ],
    },
    {
        "name": "chardev-add/remove null",
        "commands": [
            "chardev-add null,id=test-chr-add0",
            "chardev-remove test-chr-add0",
        ],
    },
    {
        "name": "chardev-change not found",
        "commands": [
            "chardev-change nosuchchardev null",
        ],
    },
    {
        "name": "chardev-send-break",
        "commands": [
            "chardev-send-break ringbuf0",
            "chardev-send-break nosuchchardev",
        ],
    },
    {
        "name": "chardev-remove not found",
        "commands": [
            "chardev-remove nosuchchardev",
        ],
    },
    {
        "name": "set_vcpu_dirty_limit negative",
        "commands": [
            "set_vcpu_dirty_limit -1",
        ],
    },
    {
        "name": "set_vcpu_dirty_limit with cpu_index",
        "commands": [
            "set_vcpu_dirty_limit 1 0",
        ],
    },
    {
        "name": "cancel_vcpu_dirty_limit",
        "commands": [
            "cancel_vcpu_dirty_limit",
        ],
    },
    {
        "name": "calc_dirty_rate zero period",
        "commands": [
            "calc_dirty_rate 0",
        ],
    },
    {
        "name": "calc_dirty_rate -r -b conflict",
        "commands": [
            "calc_dirty_rate -r -b 1",
        ],
    },
    {
        "name": "block_resize",
        "commands": [
            "block_resize snap0 20M",
            "block_resize nosuchdev 10M",
        ],
    },
    {
        "name": "block_set_io_throttle set/reset",
        "commands": [
            "block_set_io_throttle snap0 100 200 300 400 500 600",
            "block_set_io_throttle snap0 0 0 0 0 0 0",
        ],
    },
    {
        "name": "eject errors",
        "commands": [
            "eject nosuchdev",
            "eject -f nosuchdev",
        ],
    },
    {
        "name": "commit errors",
        "commands": [
            "commit nosuchdev",
        ],
    },
    {
        "name": "set_link errors",
        "commands": [
            "set_link nosuchdev off",
        ],
    },
    {
        "name": "stop/cont/info status cycle",
        "commands": [
            "stop",
            "info status",
            "cont",
            "info status",
        ],
    },
    {
        "name": "system_wakeup not suspended",
        "commands": [
            "system_wakeup",
        ],
    },
    {
        "name": "nmi",
        "commands": [
            "nmi",
        ],
    },
    {
        "name": "one-insn-per-tb toggle",
        "commands": [
            "one-insn-per-tb on",
            "one-insn-per-tb off",
        ],
    },
    {
        "name": "block_job errors",
        "commands": [
            "block_job_cancel nosuchdev",
            "block_job_pause nosuchdev",
            "block_job_resume nosuchdev",
            "block_job_complete nosuchdev",
            "block_job_set_speed nosuchdev 0",
        ],
    },
    {
        "name": "migrate_cancel no migration",
        "commands": [
            "migrate_cancel",
        ],
    },
    {
        "name": "replay_break not active",
        "commands": [
            "replay_break 100",
        ],
    },
    {
        "name": "replay_delete_break not active",
        "commands": [
            "replay_delete_break",
        ],
    },
    {
        "name": "replay_seek not active",
        "commands": [
            "replay_seek 100",
        ],
    },
    {
        "name": "device_add/del error paths",
        "commands": [
            "device_add nosuchdriver,id=test-bad-dev",
        ],
    },
    {
        "name": "block_stream not found",
        "commands": [
            "block_stream nosuchdev",
        ],
    },
    {
        "name": "sendkey invalid key",
        "commands": [
            "sendkey nosuchkey",
        ],
    },
    {
        "name": "stop/cont single-letter aliases",
        "commands": [
            "s",
            "info status",
            "c",
            "info status",
        ],
    },
]

DESTABILIZING_TESTS = [
]
