#!/bin/sh

export PATH=${HOME}/.local/share/dfx/bin:${PATH}

make start

# Since DFX is changing the binary around the deployment, we need to take the one used after the local deployment
dfx deploy
cp .dfx/local/canisters/beacon/beacon.wasm.gz  target/wasm32-unknown-unknown/release/beacon.wasm.gz
