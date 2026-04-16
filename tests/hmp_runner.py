# SPDX-License-Identifier: GPL-2.0-or-later
"""
Test runner infrastructure for compare_hmp.py.

Provides QemuSession (context manager that starts QEMU with HMP + QMP),
output comparison, and summary reporting.
"""

import difflib
import os
import subprocess

from test_utils import wait_for_socket
from hmp_monitor import HmpMonitor, QemuHmpProcess


class QemuSession:
    """Context manager that starts a QEMU instance with HMP + QMP monitors."""

    def __init__(self, qemu_bin, qemu_hmp_bin, qemu_args, tmpdir, session_id):
        self.qemu_bin = qemu_bin
        self.qemu_hmp_bin = qemu_hmp_bin
        self.qemu_args = qemu_args
        self.hmp_sock = os.path.join(tmpdir, f"{session_id}-hmp.sock")
        self.qmp_sock = os.path.join(tmpdir, f"{session_id}-qmp.sock")
        self.proc = None
        self.hmp = None
        self.ext = None

    def __enter__(self):
        qemu_cmd = [
            self.qemu_bin,
            "-display", "none",
            "-monitor", f"unix:{self.hmp_sock},server,wait=off",
            "-qmp", f"unix:{self.qmp_sock},server,wait=off",
        ] + self.qemu_args

        self.proc = subprocess.Popen(
            qemu_cmd,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
        )

        if not wait_for_socket(self.hmp_sock):
            raise RuntimeError("HMP socket did not become available")
        if not wait_for_socket(self.qmp_sock):
            raise RuntimeError("QMP socket did not become available")

        self.hmp = HmpMonitor(self.hmp_sock)
        self.ext = QemuHmpProcess(self.qemu_hmp_bin, self.qmp_sock)
        return self

    def __exit__(self, exc_type, exc_val, exc_tb):
        if self.ext:
            self.ext.close()
        if self.hmp:
            self.hmp.close()
        if self.proc:
            self.proc.terminate()
            try:
                self.proc.wait(timeout=5)
            except subprocess.TimeoutExpired:
                self.proc.kill()
                self.proc.wait()
        return False


def compare_output(hmp_output, ext_output, label):
    """Diff two output strings.

    Returns (matched, diff_text) where matched is True if identical
    and diff_text is the formatted diff string (empty when matched).
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
        text = f"--- {label} ---\n" + "\n".join(diff) + "\n"
        return False, text
    else:
        return True, ""


def _run_command(monitor, cmd):
    """Run a command on a monitor, returning output or an error string."""
    try:
        return monitor.command(cmd)
    except Exception as e:
        label = "HMP" if isinstance(monitor, HmpMonitor) else "qemu-hmp"
        return f"[{label} ERROR: {e}]"


def _compare_command(session, cmd):
    """Run a command on both monitors and return a result dict."""
    hmp_output = _run_command(session.hmp, cmd)
    ext_output = _run_command(session.ext, cmd)
    matched, diff_text = compare_output(hmp_output, ext_output, cmd)
    return {"command": cmd, "match": matched, "diff": diff_text}


def _compare_stateful_group(session, test):
    """Run a stateful test group: all commands through HMP, then all through qemu-hmp.

    This ensures both monitors see the same state transitions, since they
    share the same QEMU instance.
    """
    hmp_outputs = []
    for cmd in test["commands"]:
        hmp_outputs.append(_run_command(session.hmp, cmd))
    ext_outputs = []
    for cmd in test["commands"]:
        ext_outputs.append(_run_command(session.ext, cmd))

    results = []
    for cmd, hmp_out, ext_out in zip(test["commands"], hmp_outputs, ext_outputs):
        matched, diff_text = compare_output(hmp_out, ext_out, cmd)
        results.append({"command": cmd, "match": matched, "diff": diff_text})
    return results


def run_tests(session, commands, stateful_tests, verbose):
    """Run all commands and stateful tests against a QemuSession.

    Returns (results, any_diff).
    """
    results = []
    any_diff = False

    for entry in commands:
        if isinstance(entry, tuple):
            cmd, skip_reason = entry
        else:
            cmd, skip_reason = entry, None

        if skip_reason:
            if verbose:
                print(f"--- {cmd} ---")
                print(f"  (skipped: {skip_reason})")
                print()
            results.append({"command": cmd, "skipped": True})
            continue

        r = _compare_command(session, cmd)
        if not r["match"]:
            any_diff = True
            print(r["diff"])
        elif verbose:
            print(f"--- {cmd} ---")
            print("  (identical)")
            print()
        results.append(r)

    for test in stateful_tests:
        group_results = _compare_stateful_group(session, test)
        group_diffs = [r["diff"] for r in group_results if not r["match"]]
        if group_diffs:
            any_diff = True
            print(f"=== Stateful: {test['name']} ===")
            for text in group_diffs:
                print(text)
        elif verbose:
            print(f"=== Stateful: {test['name']} ===")
            for r in group_results:
                print(f"--- {r['command']} ---")
                print("  (identical)")
            print()

        results.extend(group_results)

    return results, any_diff


def run_session(session_cfg, qemu_bin, qemu_hmp_bin, tmpdir, verbose):
    """Run a single test session and return results."""
    name = session_cfg["name"]
    session_id = session_cfg.get("session_id", name)
    print(f"── Session: {name} " + "─" * max(1, 54 - len(name)))
    with QemuSession(qemu_bin, qemu_hmp_bin, session_cfg["qemu_args"],
                     tmpdir, session_id) as session:
        results, _ = run_tests(
            session, session_cfg["commands"],
            session_cfg["stateful_tests"], verbose)
    return results


def print_summary(results, label=None):
    """Print and return summary of test results."""
    run = [r for r in results if not r.get("skipped")]
    skipped = [r for r in results if r.get("skipped")]
    matched = sum(1 for r in run if r["match"])
    diffed = [r for r in run if not r["match"]]
    total = len(run)
    prefix = f"  [{label}] " if label else "  "
    print(f"{prefix}{matched}/{total} commands produced identical output")
    if skipped:
        print(f"{prefix}{len(skipped)} command(s) skipped")
    if diffed:
        print(f"{prefix}{len(diffed)} command(s) with differences:")
        for r in diffed:
            print(f"{prefix}  [DIFF] {r['command']}")
    return {
        "run": total,
        "matched": matched,
        "skipped": len(skipped),
        "diffed": len(diffed),
        "all_pass": len(diffed) == 0,
    }


def print_combined_summary(all_results):
    """Print combined summary across all sessions. Returns True if all passed."""
    print()
    print("=" * 60)
    print("SUMMARY")
    print("=" * 60)
    all_pass = True
    total_run = 0
    total_match = 0
    total_skip = 0
    total_diff = 0
    for label, results in all_results.items():
        summary = print_summary(results, label)
        if not summary["all_pass"]:
            all_pass = False
        total_run += summary["run"]
        total_match += summary["matched"]
        total_skip += summary["skipped"]
        total_diff += summary["diffed"]

    print()
    print(f"  TOTAL: {total_match}/{total_run} identical, "
          f"{total_skip} skipped, {total_diff} diffs")
    return all_pass
