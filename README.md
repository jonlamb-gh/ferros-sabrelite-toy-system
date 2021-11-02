# Rust seL4 toy system built on ferros for the imx6 sabrelite platform

* [ferros](https://github.com/auxoncorp/ferros): A Rust-based userland which also adds compile-time assurances to seL4 development
* [QEMU sabrelite machine](https://qemu.readthedocs.io/en/latest/system/arm/sabrelite.html)
* [device tree](https://github.com/seL4/seL4/blob/4d0f02c029560cae0e8d93727eb17d58bcecc2ac/tools/dts/sabre.dts)
* [IMX6DQRM refman](http://cache.freescale.com/files/32bit/doc/ref_manual/IMX6DQRM.pdf)
* [HW user manual](https://boundarydevices.com/wp-content/uploads/2014/11/SABRE_Lite_Hardware_Manual_rev11.pdf)
* [HW components](https://boundarydevices.com/sabre_lite-revD.pdf)
* [imx6 platform sdk](https://github.com/flit/imx6_platform_sdk)

## Getting Started

### Dependencies

```bash
# Add the extracted toolchain's bin directory to your PATH
wget https://releases.linaro.org/components/toolchain/binaries/7.4-2019.02/arm-linux-gnueabihf/gcc-linaro-7.4.1-2019.02-i686_arm-linux-gnueabihf.tar.xz

# Might need to manually install version 6.1.0
sudo apt install qemu-system-arm

# Install seL4 python deps
# https://github.com/seL4/seL4/blob/4d0f02c029560cae0e8d93727eb17d58bcecc2ac/tools/python-deps/setup.py
pip3 install --user setuptools sel4-deps

rustup target add armv7-unknown-linux-gnueabihf

cargo install --git https://github.com/auxoncorp/selfe-sys selfe-config --bin selfe --features bin --force
```

### Buidling

Log level can be set at build-time with the `RUST_ENV` environment variable (`off`, `error`, `warn`, `info`, `debug`, `trace`).

```bash
./scripts/build.sh
```

### Simulating

```bash
./scripts/simulate.sh
```

```text
ELF-loader started on CPU: ARM Ltd. Cortex-A9 r0p0
  paddr=[20000000..20825037]
No DTB found!
Looking for DTB in CPIO archive...
Found dtb at 200e1254
Loaded dtb from 200e1254
   paddr=[10041000..1004bfff]
ELF-loading image 'kernel'
  paddr=[10000000..10040fff]
  vaddr=[e0000000..e0040fff]
  virt_entry=e0000000
ELF-loading image 'root-task'
  paddr=[1004c000..1047efff]
  vaddr=[10000..442fff]
  virt_entry=22eac
ELF loader relocated, continuing boot...
Bringing up 3 other cpus
Enabling MMU and paging
Jumping to kernel-image entry point...

Bootstrapping kernel
Booting all finished, dropped to user space

Bootstrapping kernel
Booting all finished, dropped to user space
DEBUG: [root-task] Initializing
DEBUG: [root-task] Found iomux ELF data size=2770112
DEBUG: [root-task] Found persistent-storage ELF data size=3237680
DEBUG: [root-task] Found console ELF data size=2896268
DEBUG: [root-task] Setting up iomux driver
DEBUG: [root-task] Setting up persistent-storage driver
DEBUG: [root-task] Setting up console application
DEBUG: [console] process started
INFO: [console] run 'telnet 0.0.0.0 8888' to connect to the console interface
DEBUG: [persistent-storage] process started
DEBUG: [persistent-storage] storage vaddr=0x61000 size=4096
DEBUG: [persistent-storage] scratchpad vaddr=0x62000 size=4096
DEBUG: [iomux] process started
DEBUG: [iomux] Processing request ConfigureEcSpi1
DEBUG: [persistent-storage] Configured ECSPI1 IO resp=EcSpi1Configured
```

Telnet to get at the console:
```bash
telnet 0.0.0.0 8888

> help
AVAILABLE ITEMS:
  foo <a> [ <b> ] [OPTIONS...]
  bar
  sub
  help [ <command> ]
```
