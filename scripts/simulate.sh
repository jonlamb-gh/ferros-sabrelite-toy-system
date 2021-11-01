#!/usr/bin/env bash

set -e

./scripts/mkflash.sh

selfe simulate \
    --platform sabre \
    --sel4_arch aarch32 \
    --serial-override='-serial telnet:0.0.0.0:8888,server,nowait -serial mon:stdio' \
    -- \
    -smp 4 \
    -drive if=mtd,file=target/flash/flash.bin,format=raw,id=spi,index=0,bus=0

exit 0
