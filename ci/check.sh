#!/usr/bin/env sh
set -eu

mode="${1:-rust}"
cargo_toolchain="${RUSTYSOUP_CARGO_TOOLCHAIN:-stable}"
python_bin="${RUSTYSOUP_PYTHON:-.venv/bin/python}"

cargo_with_toolchain() {
    cargo +"$cargo_toolchain" "$@"
}

case "$mode" in
    rust|all)
        ;;
    *)
        echo "usage: $0 [rust|all]" >&2
        exit 2
        ;;
esac

echo "==> Rust toolchain"
rustc +"$cargo_toolchain" --version
cargo_with_toolchain --version
cargo_with_toolchain clippy --version

echo "==> cargo fmt --check"
cargo_with_toolchain fmt --check

echo "==> cargo clippy --all-targets --all-features -- -D warnings"
cargo_with_toolchain clippy --all-targets --all-features -- -D warnings

echo "==> cargo test --all-features"
cargo_with_toolchain test --all-features

if [ "$mode" = "all" ]; then
    if [ ! -x "$python_bin" ]; then
        python_bin="python3"
    fi

    if command -v maturin >/dev/null 2>&1; then
        maturin_bin="maturin"
    elif [ -x ".venv/bin/maturin" ]; then
        maturin_bin=".venv/bin/maturin"
    else
        echo "maturin is required for '$0 all'" >&2
        exit 1
    fi

    echo "==> maturin develop --release"
    "$maturin_bin" develop --release

    echo "==> pytest"
    "$python_bin" -m pytest -q
fi
