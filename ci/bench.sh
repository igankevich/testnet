#!/bin/sh

. ./ci/preamble.sh

cargo build --release
for n in $(seq 1 10); do
    time ./target/release/testnet -n "$n" true
done
