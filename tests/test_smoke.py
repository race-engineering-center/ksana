import subprocess
import threading
import time
from pathlib import Path


def _run(
    binary: Path, *args: str, timeout: float = 5.0
) -> subprocess.CompletedProcess[bytes]:
    return subprocess.run(
        [str(binary), *args],
        capture_output=True,
        timeout=timeout,
    )


def test_help_flag(binary: Path) -> None:
    result = _run(binary, "--help")
    assert result.returncode == 0
    out = result.stdout.decode()
    assert "Record and playback simulator telemetry" in out
    assert "record" in out
    assert "play" in out
    assert "inspect" in out


def test_version_flag(binary: Path) -> None:
    result = _run(binary, "--version")
    assert result.returncode == 0
    assert b"ksana" in result.stdout


def test_record_help(binary: Path) -> None:
    result = _run(binary, "record", "--help")
    assert result.returncode == 0
    out = result.stdout.decode()
    assert "--fps" in out
    assert "--max-duration" in out


def test_play_help(binary: Path) -> None:
    result = _run(binary, "play", "--help")
    assert result.returncode == 0
    assert b"--input" in result.stdout


def test_inspect_help(binary: Path) -> None:
    result = _run(binary, "inspect", "--help")
    assert result.returncode == 0
    assert b"--input" in result.stdout


def test_record_waits_for_connection(binary: Path) -> None:
    proc = subprocess.Popen(
        [str(binary)],
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    lines: list[bytes] = []

    def _read() -> None:
        assert proc.stdout is not None
        for line in proc.stdout:
            lines.append(line)

    reader = threading.Thread(target=_read, daemon=True)
    reader.start()

    time.sleep(1.5)
    assert proc.poll() is None, "Process exited prematurely"
    proc.terminate()
    reader.join(timeout=3.0)
    proc.wait(timeout=3.0)

    output = b"".join(lines).decode()
    assert "Waiting for simulator connection..." in output


def test_invalid_subcommand(binary: Path) -> None:
    result = _run(binary, "bogus")
    assert result.returncode != 0


def test_play_missing_file(binary: Path) -> None:
    result = _run(binary, "play", "--input", "nonexistent.ksr")
    assert result.returncode != 0
