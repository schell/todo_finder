#!/bin/sh -eu

section() {
    echo "----------------- $1"
}

section "Rust Setup"

if hash rustup 2>/dev/null; then
    echo "Have rustup, skipping installation..."
else
    echo "Installing rustup..."
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
fi

. $HOME/.cargo/env
rustup update
rustup toolchain install nightly
rustup component add rustfmt --toolchain nightly-x86_64-unknown-linux-gnu

if hash rg 2>/dev/null; then
    echo "Have ripgrep, skipping installation"
else
    echo "Installing ripgrep for broadphase TODO finding..."
    cargo install ripgrep
fi
