# SPDX-License-Identifier: GPL-2.0-or-later
"""
HMP and QMP monitor protocol helpers.

Provides HmpMonitor (direct socket connection to QEMU's built-in HMP)
and QemuHmpProcess (stdin/stdout pipe to the external qemu-hmp binary).
"""

import os
import re
import select
import socket
import subprocess
import sys
import time

_ANSI_RE = re.compile(r'\x1b\[[0-9;]*[A-Za-z]|\[K|\[D')
_BACKSPACE_RE = re.compile(r'[^\x08]\x08')


def _strip_terminal_escapes(text):
    """Strip readline/terminal control sequences from HMP output."""
    text = _ANSI_RE.sub('', text)
    while '\x08' in text:
        text = _BACKSPACE_RE.sub('', text)
        text = text.lstrip('\x08')
    text = text.replace('\r', '')
    lines = text.split('\n')
    deduped = []
    for line in lines:
        stripped = line.rstrip()
        if deduped and deduped[-1].rstrip() == stripped:
            continue
        deduped.append(line)
    return '\n'.join(deduped)


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
        if b"(qemu)" not in buf:
            continue
        decoded = buf.decode("utf-8", errors="replace")
        cleaned = _strip_terminal_escapes(decoded)
        if cleaned.rstrip().endswith("(qemu)") or cleaned.endswith("(qemu) "):
            break
    decoded = buf.decode("utf-8", errors="replace")
    decoded = _strip_terminal_escapes(decoded)
    idx = decoded.rfind("(qemu)")
    if idx >= 0:
        decoded = decoded[:idx]
    return decoded


def _is_readline_echo(line, cmd):
    """Detect readline echo garbage lines."""
    stripped = line.strip()
    if not stripped:
        return False
    if len(stripped) > len(cmd) * len(cmd) + len(cmd):
        return False
    cmd_chars = set(cmd)
    line_chars = set(stripped)
    if line_chars - cmd_chars - {' '}:
        return False
    if cmd not in stripped:
        return False
    if stripped.startswith(cmd + " ") and len(stripped) > len(cmd) + 1:
        return False
    return True


class HmpMonitor:
    """Persistent connection to QEMU's HMP monitor socket."""

    def __init__(self, sock_path):
        self.sock = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
        self.sock.settimeout(5)
        self.sock.connect(sock_path)
        _read_until_prompt(self.sock)

    def command(self, cmd):
        """Send a command and return the cleaned response text."""
        self.sock.sendall((cmd + "\n").encode())
        response = _read_until_prompt(self.sock)
        lines = response.strip("\n").splitlines()
        cleaned = []
        for line in lines:
            if _is_readline_echo(line, cmd):
                continue
            cleaned.append(line)
        return "\n".join(cleaned)

    def close(self):
        self.sock.close()


class QemuHmpProcess:
    """Persistent qemu-hmp subprocess communicating via stdin pipe."""

    def __init__(self, binary, qmp_sock):
        self.proc = subprocess.Popen(
            [binary, "-s", qmp_sock],
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )
        self._stdout_fd = self.proc.stdout.fileno()
        self._read_buf = b""
        banner = self.proc.stderr.readline()
        if banner:
            sys.stderr.write(f"  qemu-hmp: {banner.decode('utf-8', errors='replace')}")

    def command(self, cmd, timeout=10):
        """Send a command and return the response text."""
        self.proc.stdin.write((cmd + "\n").encode())
        self.proc.stdin.flush()
        deadline = time.time() + timeout
        while b"\x00" not in self._read_buf:
            remaining = deadline - time.time()
            if remaining <= 0:
                raise TimeoutError(
                    f"qemu-hmp did not respond within {timeout}s for: {cmd}")
            ready, _, _ = select.select([self._stdout_fd], [], [], remaining)
            if not ready:
                raise TimeoutError(
                    f"qemu-hmp did not respond within {timeout}s for: {cmd}")
            chunk = os.read(self._stdout_fd, 4096)
            if not chunk:
                break
            self._read_buf += chunk
        idx = self._read_buf.find(b"\x00")
        if idx >= 0:
            response = self._read_buf[:idx]
            self._read_buf = self._read_buf[idx + 1:]
        else:
            response = self._read_buf
            self._read_buf = b""
        return response.decode("utf-8", errors="replace").rstrip("\n")

    def close(self):
        try:
            self.proc.stdin.close()
        except BrokenPipeError:
            pass
        self.proc.wait(timeout=5)
