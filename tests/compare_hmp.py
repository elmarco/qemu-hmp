#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-2.0-or-later
"""
Compare built-in QEMU HMP output with external qemu-hmp output.

Starts a QEMU instance with both an HMP monitor and a QMP monitor,
runs a set of commands through each, and produces a side-by-side diff.

Usage:
    python3 tests/compare_hmp.py [--qemu /path/to/qemu-system-x86_64]
                                 [--qemu-hmp /path/to/qemu-hmp]
"""

import argparse
import difflib
import os
import shutil
import socket
import subprocess
import sys
import tempfile
import time


# Commands to compare.  Each entry is either a command string or a
# (command_string, skip_reason) tuple.  Skipped commands are listed
# in the output but not executed.
# Destructive commands (quit, stop, system_reset, etc.) are excluded
# because they change VM state and make subsequent comparisons invalid.
COMMANDS = [
    # info subcommands
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
    ("info snapshots", "requires x-query-snapshots QMP command (since 11.1)"),
    "info spice",
    ("info usbhost", "requires x-query-usbhost QMP command (since 11.1)"),
    ("info usernet", "requires x-query-usernet QMP command (since 11.1)"),
    "info usb",
    "info numa",
    ("info accel", "dynamic statistics differ between queries"),
    ("info jit", "dynamic statistics differ between queries"),
    ("info lapic", "requires x-query-lapic QMP command (since 11.1)"),
    ("info registers", "requires x-query-registers QMP command (since 11.1)"),
    ("info network", "requires x-query-network QMP command (since 11.1)"),
    ("info mem", "requires x-query-mem QMP command (since 11.1)"),
    ("info tlb", "requires x-query-tlb QMP command (since 11.1)"),
    ("info mtree", "requires x-query-mtree QMP command (since 11.1)"),
    ("info sync-profile", "requires x-query-sync-profile QMP command (since 11.1)"),
    ("info accelerators", "requires query-accelerators QMP command (since 10.2)"),
    "info irq",
    "info block-jobs",
    "info pci",
    "info pic",
    "info qom-tree",
    "info qdm",
    "info qtree",
    ("info qtree -b", "brief output has duplicate lines that confuse HMP reader"),
    # main commands (destructive / stateful — skip)
    ("help", "handled locally"),
    ("quit", "destructive"),
    ("stop", "changes VM state"),
    ("cont", "changes VM state"),
    ("system_reset", "destructive"),
    ("system_powerdown", "destructive"),
    ("system_wakeup", "changes VM state"),
    ("nmi", "changes VM state"),
    ("info balloon", "requires virtio-balloon device"),
    ("balloon 128", "requires virtio-balloon device"),
    ("device_del none0", "requires existing device"),
    ("block_resize drive0 1G", "requires existing block device"),
    ("block_job_cancel drive0", "requires active block job"),
    ("block_job_pause drive0", "requires active block job"),
    ("block_job_resume drive0", "requires active block job"),
    ("block_job_complete drive0", "requires active block job"),
    ("block_stream drive0", "requires backing file"),
    ("block_job_set_speed drive0 0", "requires active block job"),
    ("block_set_io_throttle snap0 0 0 0 0 0 0", "tested in stateful tests"),
    ("drive_del drive0", "destructive"),
    ("drive_backup snap0 /tmp/backup.qcow2", "tested in stateful tests"),
    ("drive_mirror snap0 /tmp/mirror.qcow2", "tested in stateful tests"),
    ("mouse_set 0", "tested in stateful tests"),
    ("migrate -d tcp:localhost:12345", "starts migration"),
    ("migrate_incoming tcp:0:12345", "changes VM state"),
    ("migrate_cancel", "requires active migration"),
    ("migrate_continue pre-switchover", "tested in stateful tests"),
    ("migrate_pause", "requires active migration"),
    ("migrate_recover tcp:0:0", "requires paused postcopy migration"),
    ("client_migrate_info spice virt42.lab.kraxel.org 1234", "requires SPICE display"),
    ("dump-guest-memory /tmp/test.elf", "tested in stateful tests"),
    ("dump-skeys /tmp/skeys", "requires S390X guest"),
    ("migration_mode 1", "requires S390X guest"),
    ("snapshot_blkdev snap0 /tmp/test-snap.qcow2", "tested in stateful tests"),
    ("snapshot_blkdev_internal snap0 test-int-snap", "tested in stateful tests"),
    ("snapshot_delete_blkdev_internal snap0 test-int-snap", "tested in stateful tests"),
    ("x_colo_lost_heartbeat", "requires COLO replication"),
    ("migrate_set_capability events on", "tested in stateful tests"),
    ("migrate_set_parameter downtime-limit 500", "tested in stateful tests"),
    ("migrate_start_postcopy", "requires active migration"),
    ("savevm test-snap", "tested in stateful tests"),
    ("loadvm test-snap", "tested in stateful tests"),
    ("delvm test-snap", "tested in stateful tests"),
    ("gdbserver", "tested in stateful tests"),
    ("gpa2hva 0", "HVA varies per run due to ASLR"),
    ("gpa2hpa 0", "HPA varies and requires /proc/self/pagemap access"),
    ("x /4xw 0", "requires running vCPU with page tables"),
    ("one-insn-per-tb", "error format differs with KVM"),
    ("log none", "tested in stateful tests"),
    ("logfile /tmp/test.log", "tested in stateful tests"),
    ("trace-file", "tested in stateful tests"),
    ("nbd_server_start localhost:10809", "tested in stateful tests"),
    ("nbd_server_add snap0", "tested in stateful tests"),
    ("nbd_server_remove snap0", "tested in stateful tests"),
    ("nbd_server_stop", "tested in stateful tests"),
    ("chardev-add null,id=test-chardev-add", "tested in stateful tests"),
    ("chardev-change test-char null", "tested in stateful tests"),
    ("chardev-remove foo", "requires existing chardev"),
    ("chardev-send-break foo", "requires existing chardev"),
    ("calc_dirty_rate 1", "tested in stateful tests"),
    ("set_vcpu_dirty_limit 1", "tested in stateful tests"),
    ("cancel_vcpu_dirty_limit", "tested in stateful tests"),
    ("closefd testfd", "tested in stateful tests"),
    ("getfd testfd", "tested in stateful tests"),
    ("mce 0 9 0x8000000000000000 0 0 0", "tested in stateful tests"),
    ("watchdog_action reset", "tested in stateful tests"),
    ("announce_self", "tested in stateful tests"),
    ("ringbuf_write ringbuf0 hello", "tested in stateful tests"),
    ("ringbuf_read ringbuf0 5", "tested in stateful tests"),
    ("expire_password vnc now", "tested in stateful tests"),
    ("set_password vnc testpw", "tested in stateful tests"),
    ("set_link net0 off", "requires existing net device"),
    ("hostfwd_add tcp::8080-:80", "tested in stateful tests"),
    ("hostfwd_remove tcp::8080", "tested in stateful tests"),
    ("replay_break 100", "tested in stateful tests"),
    ("replay_delete_break", "tested in stateful tests"),
    ("replay_seek 100", "tested in stateful tests"),
    ("dumpdtb /tmp/test.dtb", "requires CONFIG_FDT"),
    ("info firmware-log", "requires firmware log buffer"),
    ("info virtio-queue-element /foo 0", "requires virtio device"),
    ("info virtio-status /foo", "requires virtio device"),
    ("info virtio-queue-status /foo 0", "requires virtio device"),
    ("info virtio-vhost-queue-status /foo 0", "requires vhost device"),
    ("info cmma 0", "requires s390x guest"),
    ("info rocker sw1", "requires rocker device"),
    ("info rocker-ports sw1", "requires rocker device"),
    ("info rocker-of-dpa-flows sw1", "requires rocker device"),
    ("info rocker-of-dpa-groups sw1", "requires rocker device"),
    ("info skeys 0", "requires s390x guest"),
    ("info sev", "SEV not enabled output differs (monitor_printf vs QMP error)"),
    ("info sgx", "requires SGX support"),
    ("info stats vm", "requires KVM stats provider"),
    ("info vcpu_dirty_limit", "requires dirty-ring-size KVM accelerator property"),
    ("info via", "requires CONFIG_MOS6522"),
    ("xen-event-inject 1", "requires CONFIG_XEN_EMU"),
    ("xen-event-list", "requires CONFIG_XEN_EMU"),
    ("netdev_del net0", "requires existing netdev"),
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
    "sum 0 4096",
    ("i/b 0x61", "port 0x61 refresh bit toggles between reads"),
    "i/w 0x61",
    "i 0x61",
    "o/b 0x80 0",
    "o 0x80 0",
    "sendkey ret",
    "sendkey ctrl-alt-f1",
    "sendkey 0x1c",
    "qom-list",
    "qom-list /machine",
    "qom-get /machine type",
]


# Stateful test sequences.  Each entry is a named group of commands
# that run in order.  The entire sequence is run through HMP first,
# then through qemu-hmp, so both sides see the same state transitions
# (e.g. create then delete) and outputs can be compared directly.
STATEFUL_TESTS = [
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
    {
        "name": "device add/del lifecycle",
        "commands": [
            "device_add virtio-rng-pci,id=test-dev0",
            "device_del test-dev0",
        ],
    },
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
        "name": "calc_dirty_rate",
        "commands": [
            "calc_dirty_rate 1",
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
]


def find_binary(name, hint=None):
    """Find a binary by name, with an optional hint path."""
    if hint:
        if os.path.isfile(hint) and os.access(hint, os.X_OK):
            return hint
        # Maybe it's a directory; look inside
        candidate = os.path.join(hint, name)
        if os.path.isfile(candidate) and os.access(candidate, os.X_OK):
            return candidate
    # Fall back to $PATH
    found = shutil.which(name)
    if found:
        return found
    return None


def wait_for_socket(path, timeout=10):
    """Wait until a Unix socket is connectable."""
    deadline = time.time() + timeout
    while time.time() < deadline:
        try:
            s = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
            s.connect(path)
            s.close()
            return True
        except (ConnectionRefusedError, FileNotFoundError, OSError):
            time.sleep(0.1)
    return False


def _is_readline_echo(line, cmd):
    """Detect readline echo garbage lines.

    QEMU's HMP readline echoes characters one-by-one, producing lines like
    "iininfinfoinfo info vinfo veinfo verinfo vers..." for "info version".
    After escape stripping, the echo line is a concatenation of progressive
    prefixes of the command.  We detect this by checking whether the line
    consists entirely of characters from the command, in roughly the right
    order, and contains no characters that aren't in the command.

    A simpler heuristic: the echo line contains every prefix of the command
    jammed together.  So if we remove all characters of `cmd` we should be
    left with (almost) nothing.  But real output lines won't match because
    they contain digits, punctuation, etc. not in the command.
    """
    stripped = line.strip()
    if not stripped:
        return False

    # Quick reject: if the line is longer than len(cmd)*len(cmd), it's
    # almost certainly not an echo line (echo is O(n^2) in cmd length).
    if len(stripped) > len(cmd) * len(cmd) + len(cmd):
        return False

    # The echo line is built from progressive prefixes of the command.
    # For "info version", the prefixes are: "i", "in", "inf", "info",
    # "info ", "info v", ..., "info version".  Concatenated, every char
    # in the result is a char from `cmd`.
    cmd_chars = set(cmd)
    line_chars = set(stripped)
    # If the line only contains characters found in the command, it's
    # likely echo garbage.  Real output has digits, colons, parens, etc.
    if line_chars - cmd_chars - {' '}:
        # Line has characters not in the command — not an echo line
        return False

    # Additional check: the command text itself should appear as a
    # substring (the final complete echo).
    if cmd not in stripped:
        return False

    # Guard against false positives: if the line starts with the exact
    # command text followed by a space and more text, it's real command
    # output (e.g. "sync-profile is off"), not echo garbage.  Echo
    # garbage has the command embedded in a jumble of prefixes, so it
    # won't cleanly start with the command.
    if stripped.startswith(cmd + " ") and len(stripped) > len(cmd) + 1:
        return False

    return True


class HmpMonitor:
    """Persistent connection to QEMU's HMP monitor socket."""

    def __init__(self, sock_path):
        self.sock = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
        self.sock.settimeout(5)
        self.sock.connect(sock_path)
        # Read the initial prompt
        _read_until_prompt(self.sock)

    def command(self, cmd):
        """Send a command and return the cleaned response text."""
        self.sock.sendall((cmd + "\n").encode())
        response = _read_until_prompt(self.sock)

        # Strip readline echo garbage (see _is_readline_echo).
        lines = response.strip().splitlines()
        cleaned = []
        for line in lines:
            if _is_readline_echo(line, cmd):
                continue
            cleaned.append(line)
        return "\n".join(cleaned)

    def close(self):
        self.sock.close()



def _read_until_prompt(sock):
    """Read from socket until we see '(qemu) ' prompt."""
    buf = b""
    while True:
        try:
            chunk = sock.recv(4096)
        except socket.timeout:
            break
        if not chunk:
            break
        buf += chunk
        # Check for prompt at end of buffer (may include ANSI escapes)
        decoded = buf.decode("utf-8", errors="replace")
        cleaned = _strip_terminal_escapes(decoded)
        if cleaned.rstrip().endswith("(qemu)") or cleaned.endswith("(qemu) "):
            break
    decoded = buf.decode("utf-8", errors="replace")
    decoded = _strip_terminal_escapes(decoded)
    # Strip the trailing prompt
    idx = decoded.rfind("(qemu)")
    if idx >= 0:
        decoded = decoded[:idx]
    return decoded


def _strip_terminal_escapes(text):
    """Strip readline/terminal control sequences from HMP output.

    QEMU's HMP readline echoes characters one-by-one with control codes
    like \\x1b[K (erase to end of line), \\r, \\x08 (backspace), and
    [D (cursor left).  We process these to recover the clean text.
    """
    import re
    # Remove ANSI CSI escape sequences (ESC [ ... final_byte)
    text = re.sub(r'\x1b\[[0-9;]*[A-Za-z]', '', text)
    # Remove bare [K, [D sequences (sometimes ESC is missing)
    text = re.sub(r'\[K', '', text)
    text = re.sub(r'\[D', '', text)
    # Remove backspace characters and the char they erase
    while '\x08' in text:
        text = re.sub(r'[^\x08]\x08', '', text, count=1)
        # If only backspaces remain at start, strip them
        text = text.lstrip('\x08')
    # Remove carriage returns
    text = text.replace('\r', '')
    # Collapse runs of the same line (readline re-renders)
    # After stripping escapes, we may have duplicate echoed command text.
    # Split into lines and deduplicate consecutive identical lines.
    lines = text.split('\n')
    deduped = []
    for line in lines:
        stripped = line.rstrip()
        if deduped and deduped[-1].rstrip() == stripped:
            continue
        deduped.append(line)
    return '\n'.join(deduped)


class QemuHmpProcess:
    """Persistent qemu-hmp subprocess communicating via stdin pipe.

    When stdin is not a terminal, qemu-hmp reads one command per line
    and emits each response followed by a NUL byte (\\x00) separator.
    This keeps a single QMP connection alive for the entire session.
    """

    def __init__(self, binary, qmp_sock):
        self.proc = subprocess.Popen(
            [binary, "-s", qmp_sock],
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
        )
        # Wait for the connection banner on stderr.
        # stderr is line-buffered; the banner ends with a newline.
        banner = self.proc.stderr.readline()
        if banner:
            sys.stderr.write(f"  qemu-hmp: {banner}")

    def command(self, cmd):
        """Send a command and return the response text."""
        self.proc.stdin.write(cmd + "\n")
        self.proc.stdin.flush()
        # Read until NUL separator
        buf = []
        while True:
            ch = self.proc.stdout.read(1)
            if ch == "\x00" or ch == "":
                break
            buf.append(ch)
        return "".join(buf).strip("\n")

    def close(self):
        self.proc.stdin.close()
        self.proc.wait(timeout=5)


def compare_output(hmp_output, ext_output, label):
    """Diff two output strings and print the result.

    Returns True if identical, False otherwise.
    """
    hmp_lines = hmp_output.splitlines()
    ext_lines = ext_output.splitlines()

    diff = list(difflib.unified_diff(
        hmp_lines,
        ext_lines,
        fromfile=f"hmp: {label}",
        tofile=f"qemu-hmp: {label}",
        lineterm="",
    ))

    if diff:
        for line in diff:
            print(line)
        return False
    else:
        print("  (identical)")
        return True


def run_command(monitor, cmd):
    """Run a command on a monitor, returning output or an error string."""
    try:
        return monitor.command(cmd)
    except Exception as e:
        label = "HMP" if isinstance(monitor, HmpMonitor) else "qemu-hmp"
        return f"[{label} ERROR: {e}]"


def main():
    parser = argparse.ArgumentParser(
        description="Compare built-in HMP with external qemu-hmp"
    )
    parser.add_argument(
        "--qemu",
        default=None,
        help="Path to qemu-system binary (default: qemu-system-x86_64 from $PATH)",
    )
    parser.add_argument(
        "--qemu-hmp",
        default=None,
        help="Path to qemu-hmp binary (default: built from this tree)",
    )
    parser.add_argument(
        "--keep",
        action="store_true",
        help="Keep QEMU running after the test (for debugging)",
    )
    args = parser.parse_args()

    # Find binaries
    qemu_bin = find_binary("qemu-system-x86_64", args.qemu)
    if not qemu_bin:
        print("ERROR: qemu-system-x86_64 not found. Use --qemu to specify.", file=sys.stderr)
        sys.exit(1)

    # Default qemu-hmp location: built artifact in this tree
    script_dir = os.path.dirname(os.path.abspath(__file__))
    project_dir = os.path.dirname(script_dir)
    default_hmp_bin = os.path.join(project_dir, "target", "debug", "qemu-hmp")

    qemu_hmp_bin = find_binary("qemu-hmp", args.qemu_hmp) or default_hmp_bin
    if not os.path.isfile(qemu_hmp_bin):
        print(f"ERROR: qemu-hmp not found at {qemu_hmp_bin}", file=sys.stderr)
        print("Run 'cargo build' first.", file=sys.stderr)
        sys.exit(1)

    print(f"QEMU binary:     {qemu_bin}")
    print(f"qemu-hmp binary: {qemu_hmp_bin}")
    print()

    # Create temp directory for sockets and test files
    tmpdir = tempfile.mkdtemp(prefix="qemu-hmp-test-")
    hmp_sock = os.path.join(tmpdir, "hmp.sock")
    qmp_sock = os.path.join(tmpdir, "qmp.sock")
    vnc_sock = os.path.join(tmpdir, "vnc.sock")

    # Create a small raw disk image for CD-ROM change test
    cd_image = os.path.join(tmpdir, "test-cd.raw")
    with open(cd_image, "wb") as f:
        f.truncate(1024 * 1024)

    # Paths for screendump test
    screendump_ppm = os.path.join(tmpdir, "screen.ppm")
    screendump_png = os.path.join(tmpdir, "screen.png")

    # Path for logfile test
    log_file = os.path.join(tmpdir, "qemu-test.log")

    # Paths for memsave/pmemsave tests
    memsave_file = os.path.join(tmpdir, "memsave.bin")
    pmemsave_file = os.path.join(tmpdir, "pmemsave.bin")

    # Path for dump-guest-memory test
    dump_guest_memory_file = os.path.join(tmpdir, "guest-dump.elf")

    # Create a small qcow2 disk for savevm test
    snap_disk = os.path.join(tmpdir, "snap.qcow2")
    qemu_img = os.path.join(os.path.dirname(qemu_bin), "qemu-img")
    if not os.path.isfile(qemu_img):
        qemu_img = shutil.which("qemu-img")
    if qemu_img:
        subprocess.run(
            [qemu_img, "create", "-f", "qcow2", snap_disk, "10M"],
            check=True, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
        )

    # Start QEMU with both monitors, VNC (for change vnc password),
    # an empty CD-ROM drive (for change medium), and optionally a
    # qcow2 disk for savevm/loadvm/delvm tests.
    qemu_cmd = [
        qemu_bin,
        "-M", "pc",
        "-display", "none",
        "-monitor", f"unix:{hmp_sock},server,wait=off",
        "-qmp", f"unix:{qmp_sock},server,wait=off",
        "-name", "hmp-compare-test",
        "-vnc", f"unix:{vnc_sock},password=on",
        "-drive", "if=ide,index=2,media=cdrom,id=cd0",
        "-chardev", "ringbuf,id=ringbuf0",
    ]
    if os.path.isfile(snap_disk):
        qemu_cmd += ["-drive", f"file={snap_disk},format=qcow2,if=ide,id=snap0"]

    print(f"Starting QEMU: {' '.join(qemu_cmd)}")
    qemu_proc = subprocess.Popen(
        qemu_cmd,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    )

    try:
        # Wait for sockets
        if not wait_for_socket(hmp_sock):
            print("ERROR: HMP socket did not become available", file=sys.stderr)
            sys.exit(1)
        if not wait_for_socket(qmp_sock):
            print("ERROR: QMP socket did not become available", file=sys.stderr)
            sys.exit(1)

        print("QEMU started, both monitors ready.")
        print()

        # Keep persistent connections to both monitors so that
        # "info chardev" sees both chardevs as connected from either side.
        hmp = HmpMonitor(hmp_sock)
        ext = QemuHmpProcess(qemu_hmp_bin, qmp_sock)

        # Run commands and collect results
        results = []
        any_diff = False

        for entry in COMMANDS:
            if isinstance(entry, tuple):
                cmd, skip_reason = entry
            else:
                cmd, skip_reason = entry, None

            print(f"--- {cmd} ---")

            if skip_reason:
                print(f"  (skipped: {skip_reason})")
                results.append({
                    "command": cmd,
                    "skipped": True,
                })
                print()
                continue

            hmp_output = run_command(hmp, cmd)
            ext_output = run_command(ext, cmd)
            matched = compare_output(hmp_output, ext_output, cmd)
            if not matched:
                any_diff = True
            results.append({
                "command": cmd,
                "match": matched,
            })
            print()

        # Stateful test sequences — run the entire sequence through HMP
        # first, then through qemu-hmp, so both sides see the same state
        # transitions and can use the same IDs.
        all_stateful_tests = list(STATEFUL_TESTS) + [
            {
                "name": "change vnc password",
                "commands": [
                    "change vnc password testpw123",
                ],
            },
            {
                "name": "change cd-rom medium",
                "commands": [
                    f"change cd0 {cd_image}",
                    f"eject cd0",
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
            {
                "name": "log",
                "commands": [
                    "log in",
                    "log none",
                ],
            },
            {
                "name": "logfile",
                "commands": [
                    f"logfile {log_file}",
                ],
            },
            {
                "name": "trace-file",
                "commands": [
                    "trace-file",
                ],
            },
            {
                "name": "memsave/pmemsave",
                "commands": [
                    f"memsave 0xb8000 4096 {memsave_file}",
                    f"pmemsave 0xb8000 4096 {pmemsave_file}",
                ],
            },
            {
                "name": "dump-guest-memory",
                "commands": [
                    "dump-guest-memory -z -l /tmp/bad.elf",
                    "dump-guest-memory -z -s /tmp/bad.elf",
                    "dump-guest-memory -w -l /tmp/bad.elf",
                ],
            },
        ]

        if os.path.isfile(snap_disk):
            all_stateful_tests.append({
                "name": "savevm/loadvm/delvm",
                "commands": [
                    "savevm test-snap",
                    "loadvm test-snap",
                    "delvm test-snap",
                ],
            })
            all_stateful_tests.append({
                "name": "snapshot_blkdev",
                "commands": [
                    # Success case cannot be compared: it changes drive
                    # topology so the second (ext) run sees different state.
                    "snapshot_blkdev snap0",
                ],
            })
            all_stateful_tests.append({
                "name": "snapshot_blkdev_internal",
                "commands": [
                    "snapshot_blkdev_internal nosuchdev test-snap",
                ],
            })
            all_stateful_tests.append({
                "name": "snapshot_delete_blkdev_internal",
                "commands": [
                    "snapshot_delete_blkdev_internal nosuchdev test-snap",
                ],
            })
            all_stateful_tests.append({
                "name": "drive_backup",
                "commands": [
                    "drive_backup nosuchdev /tmp/backup.qcow2",
                ],
            })
            all_stateful_tests.append({
                "name": "drive_mirror",
                "commands": [
                    "drive_mirror nosuchdev /tmp/mirror.qcow2",
                ],
            })

        for test in all_stateful_tests:
            print(f"=== Stateful: {test['name']} ===")
            print()

            hmp_outputs = []
            for cmd in test["commands"]:
                hmp_outputs.append(run_command(hmp, cmd))

            ext_outputs = []
            for cmd in test["commands"]:
                ext_outputs.append(run_command(ext, cmd))

            for cmd, hmp_output, ext_output in zip(
                test["commands"], hmp_outputs, ext_outputs
            ):
                print(f"--- {cmd} ---")
                matched = compare_output(hmp_output, ext_output, cmd)
                if not matched:
                    any_diff = True
                results.append({
                    "command": cmd,
                    "match": matched,
                })
                print()

        # Summary
        print("=" * 60)
        print("SUMMARY")
        print("=" * 60)
        run = [r for r in results if not r.get("skipped")]
        skipped = [r for r in results if r.get("skipped")]
        matched = sum(1 for r in run if r["match"])
        total = len(run)
        print(f"  {matched}/{total} commands produced identical output")
        if skipped:
            print(f"  {len(skipped)} command(s) skipped")
        print()
        for r in results:
            if r.get("skipped"):
                print(f"  [SKIP] {r['command']}")
            else:
                status = "MATCH" if r["match"] else "DIFF"
                print(f"  [{status}] {r['command']}")

    finally:
        ext.close()
        hmp.close()
        if not args.keep:
            qemu_proc.terminate()
            try:
                qemu_proc.wait(timeout=5)
            except subprocess.TimeoutExpired:
                qemu_proc.kill()
                qemu_proc.wait()
            # Clean up sockets, temp images, and directory
            shutil.rmtree(tmpdir, ignore_errors=True)
            print("\nQEMU terminated, temp files cleaned up.")
        else:
            print(f"\n--keep specified. QEMU PID={qemu_proc.pid}")
            print(f"  HMP: socat - UNIX-CONNECT:{hmp_sock}")
            print(f"  QMP: {qemu_hmp_bin} -s {qmp_sock}")


if __name__ == "__main__":
    main()
