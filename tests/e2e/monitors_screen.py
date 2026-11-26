"""The Monitors page through a real terminal (§FS-021-monitors.18.1), driven by Cargo for E2E-085.

`monitors_screen.py <ephor> <cols> <rows> <steps>` runs `<ephor> tui` on a
pseudo-terminal of that size, in the environment the Rust caller built. Each
step is `[name, keys, until]`: type the keys, then wait until the current
screen contains `until`. After every step it prints `=== <name>` and the
screen as a person sees it, read back from the pyte grid rather than from the
byte stream. It exits with the program's own status once the last step has
made it exit, 2 when a step's screen never came, and 124 when the deadline
came first. The dependency is in tests/requirements.txt and CI.
"""
import codecs
import fcntl
import json
import os
import pty
import select
import struct
import subprocess
import sys
import termios
import time

import pyte

binary, cols, rows, steps = sys.argv[1], int(sys.argv[2]), int(sys.argv[3]), json.loads(sys.argv[4])
master, slave = pty.openpty()
fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack("HHHH", rows, cols, 0, 0))


def terminal_session():
    os.setsid()
    fcntl.ioctl(0, termios.TIOCSCTTY, 0)


env = dict(os.environ, TERM="xterm-256color", COLUMNS=str(cols), LINES=str(rows))
process = subprocess.Popen([binary, "tui"], env=env, stdin=slave, stdout=slave, stderr=slave,
                           preexec_fn=terminal_session)
os.close(slave)
screen = pyte.Screen(cols, rows)
stream = pyte.Stream(screen)
decoder = codecs.getincrementaldecoder("utf-8")("replace")
deadline = time.monotonic() + 40


def current():
    return "\n".join(line.rstrip() for line in screen.display)


def pump(until):
    """Read until the screen contains `until`, or the program exits."""
    while time.monotonic() < deadline:
        ready, _, _ = select.select([master], [], [], 0.05)
        if ready:
            try:
                data = os.read(master, 65536)
            except OSError:
                return until is None
            if not data:
                return until is None
            if b"\x1b[6n" in data:
                os.write(master, b"\x1b[1;1R")
            stream.feed(decoder.decode(data))
        if until is not None and until in current():
            # One more beat, so a frame drawn in two writes is whole.
            time.sleep(0.2)
            while select.select([master], [], [], 0)[0]:
                try:
                    data = os.read(master, 65536)
                except OSError:
                    break
                if not data:
                    break
                stream.feed(decoder.decode(data))
            return True
        if process.poll() is not None:
            return until is None
    return False


status = 0
for name, keys, until in steps:
    if keys:
        os.write(master, keys.encode())
    seen = pump(until)
    print("=== " + name)
    print(current())
    if not seen:
        print("=== the screen never said " + repr(until))
        status = 2
        break

try:
    code = process.wait(timeout=max(deadline - time.monotonic(), 0.1)) if status == 0 else None
except subprocess.TimeoutExpired:
    code = 124
if code is None or code == 124:
    process.terminate()
    process.wait(timeout=2)
os.close(master)
sys.exit(status or code)
