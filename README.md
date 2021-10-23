
See [QEMU sabrelite machine](https://qemu.readthedocs.io/en/latest/system/arm/sabrelite.html).

```bash
wget https://releases.linaro.org/components/toolchain/binaries/7.4-2019.02/arm-linux-gnueabihf/gcc-linaro-7.4.1-2019.02-i686_arm-linux-gnueabihf.tar.xz

cargo install --git https://github.com/auxoncorp/selfe-sys selfe-config --bin selfe --features bin --force
```

```bash
selfe build --platform sabre --sel4_arch aarch32
```

```bash
selfe simulate --platform sabre --sel4_arch aarch32 -- -smp 4
```
