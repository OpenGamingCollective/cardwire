# Packaging

This is a note for packagers trying to package cardwire

All cardwire crates can be built with rustc stable, excepted cardwire-ebpf.

## RUSTC And BPF Target

cardwire-ebpf is built by cardwire-ebpf-userspace, and require either rustc nightly (BPF target is tier 3) or enabling the bpf target directly in rustc

With a rustc built with bpf enabled:

```bash
'RUSTC_BOOTSTRAP=1 cargo build -p cardwire-ebpf-userspace'
```

Should work

## LLVM

Another issue that can happen while trying to build cardwire-ebpf-userspace is having bpf-linker errors.

Types of error i encountered:

### **memset error**

This one happens when using a bpf-linker either statically linked to llvm <=22

```bash
❯ strings $(which bpf-linker) | grep -m3 -iE "LLVM version"
LLVM version 22.1.8
```

or dynamically linked:

```bash
test@archlinux ~> ldd $(which bpf-linker)
    linux-vdso.so.1 (0x00007f73bb2fd000)
    libLLVM.so.22.1 => /usr/lib/libLLVM.so.22.1 (0x00007f73b0a00000)
    libgcc_s.so.1 => /usr/lib/libgcc_s.so.1 (0x00007f73bb2a7000)
    libc.so.6 => /usr/lib/libc.so.6 (0x00007f73b0600000)
    /lib64/ld-linux-x86-64.so.2 => /usr/lib64/ld-linux-x86-64.so.2 (0x00007f73bb2ff000)
    libffi.so.8 => /usr/lib/libffi.so.8 (0x00007f73bb299000)
    libedit.so.0 => /usr/lib/libedit.so.0 (0x00007f73bb25d000)
    libz.so.1 => /usr/lib/libz.so.1 (0x00007f73bb240000)
    libzstd.so.1 => /usr/lib/libzstd.so.1 (0x00007f73baf1a000)
    libxml2.so.16 => /usr/lib/libxml2.so.16 (0x00007f73b08ca000)
    libstdc++.so.6 => /usr/lib/libstdc++.so.6 (0x00007f73b0200000)
    libm.so.6 => /usr/lib/libm.so.6 (0x00007f73b00c9000)
    libncursesw.so.6 => /usr/lib/libncursesw.so.6 (0x00007f73baea9000)
    libicuuc.so.78 => /usr/lib/libicuuc.so.78 (0x00007f73afe00000)
    libicudata.so.78 => /usr/lib/libicudata.so.78 (0x00007f73ade00000)
```

To fix this issue, please use a bpf-linker that's linked to LLVM 23
