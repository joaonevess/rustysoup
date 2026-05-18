from pathlib import Path


def test_release_profile_keeps_unwind_panics_for_python_safety():
    cargo_toml = Path(__file__).resolve().parents[1] / "Cargo.toml"

    assert 'panic = "abort"' not in cargo_toml.read_text()
