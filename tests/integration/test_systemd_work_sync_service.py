from pathlib import Path
import unittest


ROOT = Path(__file__).resolve().parents[2]
SERVICE = ROOT / "systemd" / "ephor-work-sync.service"


def service_section() -> dict[str, list[str]]:
    properties: dict[str, list[str]] = {}
    section = ""
    for raw_line in SERVICE.read_text(encoding="utf-8").splitlines():
        line = raw_line.strip()
        if line.startswith("[") and line.endswith("]"):
            section = line[1:-1]
        elif section == "Service" and line and not line.startswith("#"):
            key, separator, value = line.partition("=")
            if separator:
                properties.setdefault(key, []).append(value)
    return properties


class WorkSyncServiceTests(unittest.TestCase):
    # §FS-005-dispatch.20: a detached run outlives its launching service.
    # §FS-005-dispatch.24: both ordered autorun positions preserve that run.
    def test_shipped_oneshot_preserves_detached_runs(self) -> None:
        service = service_section()

        self.assertEqual(service.get("Type"), ["oneshot"])
        self.assertEqual(
            service.get("KillMode"),
            ["process"],
            "§FS-005-dispatch.20 requires the service to leave detached runs alive",
        )
        self.assertEqual(
            service.get("ExecStart"),
            [
                "%h/.cargo/bin/ephor refresh --quiet",
                "%h/.cargo/bin/ephor work sync --act",
                "%h/.cargo/bin/ephor work run --due --act",
            ],
            "§FS-005-dispatch.24 requires the ordered refresh, sync, and due sweep",
        )


if __name__ == "__main__":
    unittest.main()
