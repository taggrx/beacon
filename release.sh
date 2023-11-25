#!/bin/sh

make build
make start
dfx deploy
dfx stop
