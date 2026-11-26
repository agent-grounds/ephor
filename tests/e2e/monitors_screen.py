"""`ephor tui` through a real terminal, driven by Cargo: the Monitors page
(§FS-021-monitors.18.1) for E2E-085, and the frame drawn while the session
loads (§FS-019-waiting.5.1) for E2E-088.

`monitors_screen.py <ephor> <cols> <rows> <steps>` runs `<ephor> tui` on a
pseudo-terminal of that size, in the environment the Rust caller built. Each
step is `[name, keys, until]`, or `[name, keys, until, touch]`: create the
file `touch` names, if the step names one, type the keys, then wait until the
current screen contains `until`. After every step it prints `=== <name>` and
the screen as a person sees it, read back from the pyte grid rather than from
the byte stream. Once the program has exited it prints `=== restored` and
whether the program left the alternate screen it entered. It exits with the
program's own status once the last step has made it exit, 2 when a step's
screen never came, and 124 when the deadline came first. The dependency is in
tests/requirements.txt and CI.
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
# Every byte the program wrote, to say whether it left the alternate screen.
written = bytearray()


def current():
    return "\n".join(line.rstrip() for line in screen.display)


def drain():
    """Read whatever the program left to read once it has exited."""
    while select.select([master], [], [], 0.1)[0]:
        try:
            data = os.read(master, 65536)
        except OSError:
            return
        if not data:
            return
        written.extend(data)
        stream.feed(decoder.decode(data))


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
            written.extend(data)
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
                written.extend(data)
                stream.feed(decoder.decode(data))
            return True
        if process.poll() is not None:
            drain()
            return until is None or until in current()
    return False


status = 0
for name, keys, until, *touch in steps:
    for path in touch:
        open(path, "w").close()
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
# Whatever it wrote on the way out, then whether its last word on the
# alternate screen was to leave it.
drain()
entered, left = written.rfind(b"\x1b[?1049h"), written.rfind(b"\x1b[?1049l")
print("=== restored")
print("yes" if left > entered else "no" if entered >= 0 else "never entered")
os.close(master)
sys.exit(status or code)
