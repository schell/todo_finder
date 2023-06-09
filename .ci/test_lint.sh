#!/bin/sh -eu

echo "running test_lint.sh"

ROOT="$(git rev-parse --show-toplevel)"
echo $ROOT/.ci/common.sh
. $ROOT/.ci/common.sh

section "Test"
rustup run stable cargo test --release --verbose

section "Lint"
rustup run stable cargo fmt -- --check

section "done :tada:"
