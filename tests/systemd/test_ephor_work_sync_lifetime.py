#!/usr/bin/env python3
"""Exercise §FS-005-dispatch.20 and §FS-005-dispatch.24 under systemd."""

from __future__ import annotations

import fcntl
import os
from pathlib import Path
import shlex
import shutil
import signal
import subprocess
import sys
import tempfile
import time
import uuid


ROOT = Path(__file__).resolve().parents[2]
SOURCE_UNIT = ROOT / "systemd" / "ephor-work-sync.service"
BUSCTL = ["busctl", "--user"]
MANAGER_DESTINATION = "org.freedesktop.systemd1"
MANAGER_PATH = "/org/freedesktop/systemd1"
MANAGER_INTERFACE = "org.freedesktop.systemd1.Manager"
WAIT_SECONDS = 8.0
CHILD_WAIT_SECONDS = 30.0
EXPECTED_COMMANDS = [
    "refresh --quiet",
    "work sync --act",
    "work run --due --act",
]


class InfrastructureError(RuntimeError):
    """The user manager or synchronized probe could not exercise the policy."""


def append_locked(path: Path, line: str) -> None:
    with path.open("a", encoding="utf-8") as handle:
        fcntl.flock(handle, fcntl.LOCK_EX)
        handle.write(line)
        handle.flush()
        fcntl.flock(handle, fcntl.LOCK_UN)


def allocate_invocation(state: Path) -> int:
    counter = state / "invocation-counter"
    with counter.open("a+", encoding="utf-8") as handle:
        fcntl.flock(handle, fcntl.LOCK_EX)
        handle.seek(0)
        value = int(handle.read().strip() or "0") + 1
        handle.seek(0)
        handle.truncate()
        handle.write(f"{value}\n")
        handle.flush()
        (state / "current-invocation").write_text(f"{value}\n", encoding="utf-8")
        fcntl.flock(handle, fcntl.LOCK_UN)
    return value


def wait_for(predicate, description: str, timeout: float = WAIT_SECONDS) -> None:
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        if predicate():
            return
        time.sleep(0.05)
    raise InfrastructureError(f"timed out waiting for {description}")


def run_child(state: Path, invocation: int, phase: str) -> int:
    append_locked(state / "children", f"{os.getpid()} {invocation} {phase}\n")
    (state / f"ack-{invocation}-{phase}").write_text("launched\n", encoding="utf-8")
    release = state / f"release-{invocation}-{phase}"
    deadline = time.monotonic() + CHILD_WAIT_SECONDS
    while time.monotonic() < deadline:
        if release.exists():
            (state / f"complete-{invocation}-{phase}").write_text(
                "finished\n", encoding="utf-8"
            )
            return 0
        time.sleep(0.05)
    (state / f"child-timeout-{invocation}-{phase}").write_text(
        "release was not observed\n", encoding="utf-8"
    )
    return 2


def launch_child(state: Path, invocation: int, phase: str) -> None:
    subprocess.Popen(
        [
            sys.executable,
            str(Path(__file__).resolve()),
            "--child",
            str(state),
            str(invocation),
            phase,
        ],
        stdin=subprocess.DEVNULL,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
        start_new_session=True,
    )
    try:
        wait_for(
            lambda: (state / f"ack-{invocation}-{phase}").exists(),
            f"{phase} child launch acknowledgement",
        )
    except InfrastructureError as error:
        (state / f"launch-error-{invocation}-{phase}").write_text(
            f"{error}\n", encoding="utf-8"
        )
        raise


def run_double(state: Path, arguments: list[str]) -> int:
    if arguments == ["refresh", "--quiet"]:
        invocation = allocate_invocation(state)
    else:
        current = state / "current-invocation"
        wait_for(current.exists, "refresh invocation record")
        invocation = int(current.read_text(encoding="utf-8").strip())

    command = " ".join(arguments)
    append_locked(state / "commands", f"{invocation} {command}\n")
    if arguments == ["work", "sync", "--act"]:
        launch_child(state, invocation, "sync")
    elif arguments == ["work", "run", "--due", "--act"]:
        launch_child(state, invocation, "due")
        mode = (state / "mode").read_text(encoding="utf-8").strip()
        if invocation == 1 and mode in {"stop", "restart"}:
            deadline = time.monotonic() + CHILD_WAIT_SECONDS
            while time.monotonic() < deadline:
                time.sleep(0.1)
            return 2
    return 0


def bus_command(*arguments: str, timeout: float = WAIT_SECONDS) -> subprocess.CompletedProcess[str]:
    try:
        return subprocess.run(
            [*BUSCTL, *arguments],
            text=True,
            capture_output=True,
            timeout=timeout,
            check=False,
        )
    except subprocess.TimeoutExpired as error:
        raise InfrastructureError(f"busctl {' '.join(arguments)} timed out") from error


def manager_call(method: str, signature: str = "", *arguments: str) -> None:
    command = [
        "call",
        MANAGER_DESTINATION,
        MANAGER_PATH,
        MANAGER_INTERFACE,
        method,
    ]
    if signature:
        command.extend([signature, *arguments])
    result = bus_command(*command)
    if result.returncode != 0:
        raise InfrastructureError(f"manager {method} failed: {result.stderr.strip()}")


def unit_path(unit: str) -> str:
    result = bus_command(
        "call",
        MANAGER_DESTINATION,
        MANAGER_PATH,
        MANAGER_INTERFACE,
        "LoadUnit",
        "s",
        unit,
    )
    if result.returncode != 0:
        raise InfrastructureError(f"manager could not load {unit}: {result.stderr.strip()}")
    fields = shlex.split(result.stdout)
    if len(fields) != 2 or fields[0] != "o":
        raise InfrastructureError(f"manager returned an invalid path for {unit}: {result.stdout.strip()}")
    return fields[1]


def unit_property(unit: str, interface: str, property_name: str) -> str:
    result = bus_command(
        "get-property",
        MANAGER_DESTINATION,
        unit_path(unit),
        interface,
        property_name,
    )
    if result.returncode != 0:
        raise InfrastructureError(
            f"manager could not read {property_name} for {unit}: {result.stderr.strip()}"
        )
    fields = shlex.split(result.stdout)
    if len(fields) != 2:
        raise InfrastructureError(
            f"manager returned an invalid {property_name} for {unit}: {result.stdout.strip()}"
        )
    return fields[1]


def manager_state(unit: str) -> str:
    active = unit_property(unit, "org.freedesktop.systemd1.Unit", "ActiveState")
    sub = unit_property(unit, "org.freedesktop.systemd1.Unit", "SubState")
    result = unit_property(unit, "org.freedesktop.systemd1.Service", "Result")
    return f"ActiveState={active}, SubState={sub}, Result={result}"


def wait_inactive(unit: str) -> None:
    def inactive() -> bool:
        return unit_property(unit, "org.freedesktop.systemd1.Unit", "ActiveState") in {
            "inactive",
            "failed",
        }

    wait_for(inactive, f"{unit} to become inactive")


def source_kill_policy() -> list[str]:
    return [
        line.strip()
        for line in SOURCE_UNIT.read_text(encoding="utf-8").splitlines()
        if line.strip().startswith("KillMode=")
    ]


def derive_unit(destination: Path, state: Path) -> None:
    executable = "%h/.cargo/bin/ephor"
    replacement = f"{sys.executable} {Path(__file__).resolve()} --double {state}"
    source = SOURCE_UNIT.read_text(encoding="utf-8")
    if source.count(f"ExecStart={executable}") != 3:
        raise InfrastructureError("the shipped unit does not have the expected three commands")
    derived = source.replace(f"ExecStart={executable}", f"ExecStart={replacement}")
    destination.write_text(derived, encoding="utf-8")
    derived_policy = [
        line.strip()
        for line in derived.splitlines()
        if line.strip().startswith("KillMode=")
    ]
    if derived_policy != source_kill_policy():
        raise InfrastructureError("the derived unit did not preserve the shipped kill policy")


def expected_acknowledgements(invocations: int) -> list[str]:
    return [
        f"ack-{invocation}-{phase}"
        for invocation in range(1, invocations + 1)
        for phase in ("sync", "due")
    ]


def expected_completions(invocations: int) -> list[str]:
    return [name.replace("ack-", "complete-") for name in expected_acknowledgements(invocations)]


def wait_for_files(state: Path, names: list[str], description: str) -> None:
    def all_exist() -> bool:
        return all((state / name).exists() for name in names)

    try:
        wait_for(all_exist, description)
    except InfrastructureError:
        launch_errors = sorted(state.glob("launch-error-*"))
        if launch_errors:
            details = "; ".join(path.read_text(encoding="utf-8").strip() for path in launch_errors)
            raise InfrastructureError(f"launch handshake failed: {details}")
        raise


def read_commands(state: Path) -> list[str]:
    path = state / "commands"
    if not path.exists():
        return []
    return path.read_text(encoding="utf-8").splitlines()


def release_children(state: Path, invocations: int) -> None:
    for name in expected_acknowledgements(invocations):
        (state / name.replace("ack-", "release-")).write_text("finish\n", encoding="utf-8")


def clean_children(state: Path) -> None:
    children = state / "children"
    if not children.exists():
        return
    for line in children.read_text(encoding="utf-8").splitlines():
        try:
            pid = int(line.split()[0])
            os.kill(pid, signal.SIGTERM)
        except (ProcessLookupError, PermissionError, ValueError, IndexError):
            pass


def check_order(state: Path, invocations: int) -> list[str]:
    expected = [
        f"{invocation} {command}"
        for invocation in range(1, invocations + 1)
        for command in EXPECTED_COMMANDS
    ]
    actual = read_commands(state)
    if actual == expected:
        return []
    return [f"command order was {actual!r}, expected {expected!r}"]


def run_scenario(root: Path, mode: str) -> list[str]:
    state = root / mode
    state.mkdir()
    (state / "mode").write_text(f"{mode}\n", encoding="utf-8")
    token = uuid.uuid4().hex[:10]
    unit = f"ephor-work-sync-lifetime-{token}-{mode}.service"
    unit_path = state / unit
    derive_unit(unit_path, state)
    invocations = 2 if mode == "restart" else 1

    manager_call("LinkUnitFiles", "asbb", "1", str(unit_path), "true", "true")
    manager_call("Reload")

    try:
        manager_call("StartUnit", "ss", unit, "replace")
        first_acks = expected_acknowledgements(1)
        wait_for_files(state, first_acks, f"both {mode} launch acknowledgements")
        before = manager_state(unit)
        print(f"scenario={mode} acknowledgements={','.join(first_acks)} manager-before={before}")

        if mode == "stop":
            manager_call("StopUnit", "ss", unit, "replace")
        elif mode == "restart":
            manager_call("RestartUnit", "ss", unit, "replace")
            second_acks = expected_acknowledgements(2)[2:]
            wait_for_files(state, second_acks, "both restarted launch acknowledgements")

        wait_inactive(unit)
        after = manager_state(unit)
        expected_stop_result = mode == "stop" and "Result=signal" in after
        if not expected_stop_result and (
            "ActiveState=failed" in after or "Result=success" not in after
        ):
            raise InfrastructureError(f"service invocation failed: {after}")
        commands = read_commands(state)
        print(f"scenario={mode} commands={' | '.join(commands)} manager-after={after}")
        release_children(state, invocations)

        completions = expected_completions(invocations)
        deadline = time.monotonic() + WAIT_SECONDS
        while time.monotonic() < deadline:
            if all((state / name).exists() for name in completions):
                break
            time.sleep(0.05)
        missing = [name for name in completions if not (state / name).exists()]
        failures = check_order(state, invocations)
        if missing:
            failures.append(
                "detached children did not survive service "
                f"{mode}: missing completion markers {', '.join(missing)}"
            )
        return failures
    finally:
        try:
            manager_call("StopUnit", "ss", unit, "replace")
        except InfrastructureError:
            pass
        clean_children(state)
        try:
            manager_call("DisableUnitFiles", "asb", "1", unit, "true")
        except InfrastructureError:
            pass
        try:
            manager_call("ResetFailedUnit", "s", unit)
        except InfrastructureError:
            pass
        try:
            manager_call("Reload")
        except InfrastructureError:
            pass


def run_controller() -> int:
    if not SOURCE_UNIT.is_file():
        print(f"INFRASTRUCTURE ERROR: shipped unit is missing: {SOURCE_UNIT}", file=sys.stderr)
        return 2
    if shutil.which("busctl") is None:
        print("INFRASTRUCTURE ERROR: busctl is not installed", file=sys.stderr)
        return 2
    available = bus_command(
        "get-property",
        MANAGER_DESTINATION,
        MANAGER_PATH,
        MANAGER_INTERFACE,
        "Version",
    )
    if available.returncode != 0:
        print(
            f"INFRASTRUCTURE ERROR: no usable systemd user manager: {available.stderr.strip()}",
            file=sys.stderr,
        )
        return 2

    scratch = Path.home() / "ag" / "tmp"
    scratch.mkdir(parents=True, exist_ok=True)
    policy = source_kill_policy()
    print(f"checkout={ROOT} commit={subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=ROOT, text=True).strip()}")
    print(f"source-policy={','.join(policy) if policy else '(systemd default)'}")
    failures: list[str] = []
    try:
        with tempfile.TemporaryDirectory(prefix="ephor-work-sync-lifetime-", dir=scratch) as tmp:
            temp_root = Path(tmp)
            for mode in ("completion", "stop", "restart"):
                scenario_failures = run_scenario(temp_root, mode)
                for failure in scenario_failures:
                    failures.append(f"{mode}: {failure}")
    except InfrastructureError as error:
        print(f"INFRASTRUCTURE ERROR: {error}", file=sys.stderr)
        return 2

    if failures:
        for failure in failures:
            print(f"REGRESSION FAILURE: {failure}", file=sys.stderr)
        return 1
    print("all detached sync and due children survived completion, stop, and restart")
    return 0


def main(arguments: list[str]) -> int:
    if len(arguments) >= 1 and arguments[0] == "--child":
        return run_child(Path(arguments[1]), int(arguments[2]), arguments[3])
    if len(arguments) >= 1 and arguments[0] == "--double":
        return run_double(Path(arguments[1]), arguments[2:])
    if arguments:
        print(f"usage: {Path(sys.argv[0]).name}", file=sys.stderr)
        return 2
    return run_controller()


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
