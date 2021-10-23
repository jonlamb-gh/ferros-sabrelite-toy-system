# Rust seL4 toy system built on ferros for the imx6 sabrelite platform

* [QEMU sabrelite machine](https://qemu.readthedocs.io/en/latest/system/arm/sabrelite.html)
* [device tree](https://github.com/seL4/seL4/blob/4d0f02c029560cae0e8d93727eb17d58bcecc2ac/tools/dts/sabre.dts)

```bash
# Add to PATH
wget https://releases.linaro.org/components/toolchain/binaries/7.4-2019.02/arm-linux-gnueabihf/gcc-linaro-7.4.1-2019.02-i686_arm-linux-gnueabihf.tar.xz

cargo install --git https://github.com/auxoncorp/selfe-sys selfe-config --bin selfe --features bin --force
```

```bash
# ./build.sh
selfe build --platform sabre --sel4_arch aarch32
```

```bash
# ./simulate.sh
selfe simulate --platform sabre --sel4_arch aarch32 -- -smp 4
```
