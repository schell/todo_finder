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
    echo "Installing ripgrep..."
    cargo install ripgrep
fi

# AWS CLI
if hash aws 2>/dev/null; then
    echo "Have aws cli, skipping installation..."
else
    echo "Installing aws cli..."
    curl "https://awscli.amazonaws.com/awscli-exe-linux-x86_64.zip" -o "awscliv2.zip"
    unzip awscliv2.zip
    sudo ./aws/install
    echo "Installed aws into:"
    which aws
fi

# TERRAFORM
if hash terraform 2>/dev/null; then
    echo "Terraform is already installed at:"
    which terraform
else
    VER="0.12.21"
    PKG="terraform_${VER}_linux_amd64"
    DIR="$HOME/.local/bin"
    PREV=`pwd`
    echo "Installing terraform..."
    mkdir -p install-terraform
    cd install-terraform
    echo "  downloading terraform..."
    wget https://releases.hashicorp.com/terraform/$VER/$PKG.zip
    echo "  unzipping..."
    unzip $PKG
    echo "  moving it into place..."
    mv terraform $DIR/
    echo "  cleaning up..."
    rm $PKG.zip
    cd $PREV
    tpath=`which terraform`
    echo "  installed terraform into: ${tpath}"
fi
