#!/bin/sh

. ./ci/preamble.sh

test_all() {
    cargo test --workspace --quiet --no-run "$@"
    cargo test --workspace --no-fail-fast "$@" -- --nocapture
}

test_all --no-default-features
