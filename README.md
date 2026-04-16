# qemu-hmp

Standalone HMP monitor for QEMU, communicating exclusively through QMP.

When QAPI launched 15 years ago, the plan was to migrate every HMP command over
to QMP. Fast forward to today, and that goal is still a work in progress—mostly
because manual conversion isn't exactly a "fun" task. I decided to see if LLMs
could handle the heavy lifting. After a few hours of prompting Claude to
replicate HMP via the QMP API, I was able to quickly map out the missing
pieces. This is the result of this experiment.

## Features

- "reedline" support, with history, bindings etc
- QOM events
- completions, including QOM tree, block devices, enum values, keyval
- inline "kitty" screendump
- aiming at feature parity

## Building

```sh
cargo build
```

To build against a different QEMU source tree:

```sh
QEMU_SCHEMA_DIR=/path/to/qemu cargo build
```

## Usage

```sh
# Start QEMU with a QMP socket:
qemu-system-x86_64 ... -qmp unix:/tmp/qmp.sock,server,wait=off

# Interactive mode:
qemu-hmp -s /tmp/qmp.sock

# Batch mode:
qemu-hmp -s /tmp/qmp.sock -c "info version" -c "info cpus"

# Pipe mode (stdin is not a terminal):
echo "info version" | qemu-hmp -s /tmp/qmp.sock
```

Pipe mode emits a NUL byte (`\0`) after each response for machine parsing.

## Testing

```sh
cargo test
```

To verify output parity with QEMU's built-in HMP:

```sh
python3 tests/compare_hmp.py [--qemu /path/to/qemu-system-x86_64]
```

## License

GPL-2.0-or-later
