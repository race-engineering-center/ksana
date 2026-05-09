from pathlib import Path

import pytest

_BASE = Path(__file__).parent.parent
_PROFILES = ("debug", "release")


def pytest_generate_tests(metafunc: pytest.Metafunc) -> None:
    if "binary" not in metafunc.fixturenames:
        return
    builds = [
        _BASE / "target" / profile / "ksana.exe"
        for profile in _PROFILES
        if (_BASE / "target" / profile / "ksana.exe").exists()
    ]
    if not builds:
        pytest.fail("No ksana builds found")
    ids = [p.parts[-2] for p in builds]  # "debug" or "release"
    metafunc.parametrize("binary", builds, ids=ids, indirect=True)


@pytest.fixture
def binary(request: pytest.FixtureRequest) -> Path:
    return request.param
