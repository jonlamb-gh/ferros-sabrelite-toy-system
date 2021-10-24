set -e

selfe simulate \
    --platform sabre \
    --sel4_arch aarch32 \
    --serial-override='-serial telnet:0.0.0.0:8888,server,nowait -serial mon:stdio' \
    -- -smp 4

exit 0
