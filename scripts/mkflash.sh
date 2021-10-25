#!/usr/bin/env bash

set -e

FILE="target/flash/flash.bin"
BASEDIR=$(dirname "$FILE")

mkdir -p "$BASEDIR"

if [ ! -f "$FILE" ]; then
    dd if=/dev/zero of="$FILE" bs=1M count=64
fi

exit 0
