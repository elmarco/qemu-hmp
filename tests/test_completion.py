#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-2.0-or-later
"""
Test tab-completion against a real QEMU instance.

Starts QEMU with a QMP socket and uses `qemu-hmp --complete` to verify
that completion suggestions are correct for object types, property names,
and enum values.

Usage:
    python3 tests/test_completion.py [--qemu /path/to/qemu-system-x86_64]
                                     [--qemu-hmp /path/to/qemu-hmp]
"""

import argparse
import os
import shutil
import socket
import subprocess
import sys
import tempfile
import time


def find_binary(name, hint=None):
    """Find a binary by name, with an optional hint path."""
    if hint:
        if os.path.isfile(hint) and os.access(hint, os.X_OK):
            return hint
        candidate = os.path.join(hint, name)
        if os.path.isfile(candidate) and os.access(candidate, os.X_OK):
            return candidate
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


def get_completions(qemu_hmp_bin, qmp_sock, line):
    """Run qemu-hmp --complete and return the list of suggestions."""
    result = subprocess.run(
        [qemu_hmp_bin, "-s", qmp_sock, "--complete", line],
        capture_output=True,
        text=True,
        timeout=10,
    )
    if result.returncode != 0:
        print(f"  STDERR: {result.stderr.strip()}", file=sys.stderr)
        return []
    return [s for s in result.stdout.strip().splitlines() if s]


def main():
    parser = argparse.ArgumentParser(
        description="Test qemu-hmp tab-completion against a real QEMU"
    )
    parser.add_argument("--qemu", default=None)
    parser.add_argument("--qemu-hmp", default=None)
    args = parser.parse_args()

    qemu_bin = find_binary("qemu-system-x86_64", args.qemu)
    if not qemu_bin:
        print("ERROR: qemu-system-x86_64 not found.", file=sys.stderr)
        sys.exit(1)

    script_dir = os.path.dirname(os.path.abspath(__file__))
    project_dir = os.path.dirname(script_dir)
    default_hmp_bin = os.path.join(project_dir, "target", "debug", "qemu-hmp")
    qemu_hmp_bin = find_binary("qemu-hmp", args.qemu_hmp) or default_hmp_bin
    if not os.path.isfile(qemu_hmp_bin):
        print(f"ERROR: qemu-hmp not found at {qemu_hmp_bin}", file=sys.stderr)
        sys.exit(1)

    tmpdir = tempfile.mkdtemp(prefix="qemu-hmp-complete-")
    qmp_sock = os.path.join(tmpdir, "qmp.sock")

    qemu_cmd = [
        qemu_bin,
        "-M", "none",
        "-display", "none",
        "-qmp", f"unix:{qmp_sock},server,wait=off",
    ]

    print(f"QEMU: {qemu_bin}")
    print(f"qemu-hmp: {qemu_hmp_bin}")
    print()

    qemu_proc = subprocess.Popen(
        qemu_cmd,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    )

    passed = 0
    failed = 0

    def check(description, line, expect_fn):
        """Run a completion check. expect_fn(completions) -> (ok, detail)."""
        nonlocal passed, failed
        completions = get_completions(qemu_hmp_bin, qmp_sock, line)
        ok, detail = expect_fn(completions)
        status = "PASS" if ok else "FAIL"
        if not ok:
            failed += 1
        else:
            passed += 1
        print(f"  [{status}] {description}")
        if not ok:
            print(f"         {detail}")
            print(f"         got: {completions}")

    try:
        if not wait_for_socket(qmp_sock):
            print("ERROR: QMP socket not available", file=sys.stderr)
            sys.exit(1)
        print("QEMU started.\n")

        # --- Command name completion ---
        print("Command name completion:")
        check(
            "complete 'info' from 'in'",
            "in",
            lambda c: ("info" in c, "'info' not in completions"),
        )
        check(
            "complete 'info v' -> info subcommands starting with v",
            "info v",
            lambda c: ("version" in c, "'version' not in completions"),
        )

        # --- Object type completion ---
        print("\nObject type completion:")
        check(
            "object_add with empty prefix lists user-creatable types",
            "object_add ",
            lambda c: (
                len(c) > 0 and "rng-random" in c,
                "'rng-random' not in completions",
            ),
        )
        check(
            "object_add with prefix 'rng' filters to rng types",
            "object_add rng",
            lambda c: (
                all("rng" in t for t in c) and len(c) > 0,
                "not all completions contain 'rng'",
            ),
        )

        # --- Property name completion ---
        print("\nProperty name completion:")
        check(
            "object_add rng-random, lists property keys with '='",
            "object_add rng-random,",
            lambda c: (
                any(s.endswith("=") for s in c) and "id=" in c,
                "'id=' not in completions or no '=' suffixes",
            ),
        )
        check(
            "object_add rng-random,id=foo, excludes already-used 'id'",
            "object_add rng-random,id=foo,",
            lambda c: (
                "id=" not in c,
                "'id=' should be excluded but was present",
            ),
        )
        check(
            "object_add rng-random, excludes discriminator tag 'qom-type'",
            "object_add rng-random,",
            lambda c: (
                "qom-type=" not in c,
                "'qom-type=' should be excluded (it is the first positional)",
            ),
        )

        # --- Enum value completion ---
        # Find a type with an enum property from the schema.
        # 'memory-backend-file' has 'share' (bool) and other props.
        # 'tls-creds-x509' has 'endpoint' which is a QCryptoTLSCredsEndpoint enum.
        # Let's try a few known enum properties.
        print("\nEnum value completion:")

        # First, find the properties of a type that has an enum.
        # We'll check a few candidates.
        # 'tls-creds-x509' -> endpoint: QCryptoTLSCredsEndpoint (server, client)
        props = get_completions(qemu_hmp_bin, qmp_sock, "object_add tls-creds-x509,")
        has_endpoint = "endpoint=" in props
        if has_endpoint:
            check(
                "tls-creds-x509,endpoint= lists enum values",
                "object_add tls-creds-x509,endpoint=",
                lambda c: (
                    len(c) > 0 and "server" in c and "client" in c,
                    "expected 'server' and 'client' in enum values",
                ),
            )
            check(
                "tls-creds-x509,endpoint=s filters to 'server'",
                "object_add tls-creds-x509,endpoint=s",
                lambda c: (
                    "server" in c and "client" not in c,
                    "expected only 'server' for prefix 's'",
                ),
            )
        else:
            # Try another type — 'memory-backend-file' might not have enums,
            # but 'secret' has 'format' (QCryptoSecretFormat: raw, base64)
            props = get_completions(qemu_hmp_bin, qmp_sock, "object_add secret,")
            has_format = "format=" in props
            if has_format:
                check(
                    "secret,format= lists enum values",
                    "object_add secret,format=",
                    lambda c: (
                        len(c) > 0 and "raw" in c and "base64" in c,
                        "expected 'raw' and 'base64' in enum values",
                    ),
                )
                check(
                    "secret,format=r filters to 'raw'",
                    "object_add secret,format=r",
                    lambda c: (
                        "raw" in c and "base64" not in c,
                        "expected only 'raw' for prefix 'r'",
                    ),
                )
            else:
                print("  [SKIP] no known enum property found for testing")

        # --- Non-enum property should give no value completions ---
        print("\nNon-enum value completion:")
        check(
            "rng-random,id= gives no completions (string property)",
            "object_add rng-random,id=",
            lambda c: (len(c) == 0, "expected no completions for string property"),
        )

        # --- QOM path completion ---
        print("\nQOM path completion:")
        check(
            "qom-list with empty arg suggests '/'",
            "qom-list ",
            lambda c: ("/" in c, "'/' not in completions"),
        )
        check(
            "qom-list / lists children of root",
            "qom-list /",
            lambda c: (
                len(c) > 0 and any("machine" in s for s in c),
                "'machine' child not found under root",
            ),
        )

        # --- Summary ---
        print(f"\n{'=' * 40}")
        print(f"RESULTS: {passed} passed, {failed} failed")
        print(f"{'=' * 40}")

    finally:
        qemu_proc.terminate()
        try:
            qemu_proc.wait(timeout=5)
        except subprocess.TimeoutExpired:
            qemu_proc.kill()
            qemu_proc.wait()
        for f in [qmp_sock]:
            try:
                os.unlink(f)
            except OSError:
                pass
        try:
            os.rmdir(tmpdir)
        except OSError:
            pass

    sys.exit(1 if failed else 0)


if __name__ == "__main__":
    main()
