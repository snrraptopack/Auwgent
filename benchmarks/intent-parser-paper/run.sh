#!/usr/bin/env sh
set -eu
artifact_root=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
"$artifact_root/scripts/prepare-source.sh"
cd "$artifact_root"
cargo run --release -- "$@"
