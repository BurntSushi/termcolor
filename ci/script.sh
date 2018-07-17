#!/bin/bash

# build, test and generate docs in this phase

set -ex

main() {
    cargo build --target "$TARGET" --verbose
    cargo test --target "$TARGET" --verbose
    cargo test --target "$TARGET" --verbose --manifest-path wincolor/Cargo.toml
}

main
