#!/bin/bash

cd "$(dirname "$0")"

RUSTFLAGS="-C link-arg=-Tnrf52840dk_layout.ld -C relocation-model=static" cargo build --release --target=thumbv7em-none-eabi --features=with_ctap1
