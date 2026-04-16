#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-2.0-or-later
"""Shared test utilities for qemu-hmp test suite."""

import os
import shutil
import socket
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
