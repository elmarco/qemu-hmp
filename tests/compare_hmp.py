#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-2.0-or-later
"""
Compare built-in QEMU HMP output with external qemu-hmp output.

Starts QEMU instances with both an HMP monitor and a QMP monitor,
runs a set of commands through each, and produces a side-by-side diff.

Usage:
    python3 tests/compare_hmp.py [--qemu /path/to/qemu-system-x86_64]
                                 [--qemu-hmp /path/to/qemu-hmp]
"""

import argparse
import os
import shutil
import sys
import tempfile

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from test_utils import find_binary
from hmp_sessions import (
    build_main_session, BALLOON_SESSION, NETWORK_SESSION, USB_SESSION,
    SMP_SESSION, NUMA_SESSION, IOTHREAD_SESSION, MEMORY_HOTPLUG_SESSION,
    SPICE_SESSION, PCIE_AER_SESSION,
)
from hmp_runner import run_session, print_combined_summary


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
    parser.add_argument(
        "-v", "--verbose",
        action="store_true",
        help="Show all commands, not just those with diffs",
    )
    args = parser.parse_args()

    qemu_bin = find_binary("qemu-system-x86_64", args.qemu)
    if not qemu_bin:
        print("ERROR: qemu-system-x86_64 not found. Use --qemu to specify.",
              file=sys.stderr)
        sys.exit(1)

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

    tmpdir = tempfile.mkdtemp(prefix="qemu-hmp-test-")

    sessions = [
        build_main_session(tmpdir, qemu_bin),
        BALLOON_SESSION,
        NETWORK_SESSION,
        USB_SESSION,
        SMP_SESSION,
        NUMA_SESSION,
        IOTHREAD_SESSION,
        MEMORY_HOTPLUG_SESSION,
        SPICE_SESSION,
        PCIE_AER_SESSION,
    ]

    try:
        all_results = {}
        for session_cfg in sessions:
            if all_results:
                print()
            all_results[session_cfg["name"]] = run_session(
                session_cfg, qemu_bin, qemu_hmp_bin, tmpdir, args.verbose)

        all_pass = print_combined_summary(all_results)
    finally:
        if not args.keep:
            shutil.rmtree(tmpdir, ignore_errors=True)
            print("\nTemp files cleaned up.")

    sys.exit(0 if all_pass else 1)


if __name__ == "__main__":
    main()
