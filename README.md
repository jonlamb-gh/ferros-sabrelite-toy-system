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
TRACE: [root-task] Initializing
TRACE: [root-task] Found iomux ELF data size=2769676
TRACE: [root-task] Found persistent-storage ELF data size=2999236
TRACE: [root-task] Found console ELF data size=2895828
TRACE: [root-task] Setting up iomux driver
TRACE: [root-task] Setting up persistent-storage driver
TRACE: [root-task] Setting up console application
TRACE: [console] process started
INFO: [console] run 'telnet 0.0.0.0 8888' to connect to the console interface
TRACE: [persistent-storage] process started, storage vaddr=0x4F000 size=4096
TRACE: [iomux] process started
DEBUG: [iomux] Processing request ConfigureEcSpi1
TRACE: [iomux] PAD_EIM_D17__ECSPI1_MISO
TRACE: [iomux] PAD_EIM_D18__ECSPI1_MOSI
TRACE: [iomux] PAD_EIM_D16__ECSPI1_SCLK
TRACE: [iomux] PAD_EIM_D19__GPIO3_IO19
TRACE: [persistent-storage] Configured ECSPI1 IO resp=EcSpi1Configured
TRACE: [ECSPI1] init
TRACE: [ECSPI1] ctl=0x20F1, cfg=0x0000, period=0x8000
TRACE: [ECSPI1] ctl=0x20F1, cfg=0x0000, period=0x8000
TRACE: [ECSPI1] transfer len 1 bit_len 8
TRACE: [ECSPI1] transfer len 1 bit_len 8
TRACE: [ECSPI1] ctl=0x20F1, cfg=0x0000, period=0x8000
TRACE: [ECSPI1] transfer len 1 bit_len 8
TRACE: [ECSPI1] transfer len 6 bit_len 48
TRACE: [flash] init status=(empty) MFR=0xBF ID=0x2541
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
