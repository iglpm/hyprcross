# Testing with QEMU user mode

On Linux, `hyprcross test` and `hyprcross run` use QEMU user mode for a foreign Linux CPU.
hyprcross chooses the emulator, passes the installed SDK as its library prefix, and injects the
runner into Igel or Cargo.

Install the user-mode emulators with your distribution package manager. Common package names
are `qemu-user-static` on Debian/Ubuntu and `qemu-user` or `qemu-user-static` on Arch Linux.
`hyprcross doctor` reports the runners it finds.

```sh
hyprcross test -t aarch64-unknown-linux-gnu
hyprcross test -t riscv64gc-unknown-linux-musl
hyprcross run -t armv7-unknown-linux-gnueabihf -- --flag
```

For dynamically linked targets, QEMU must find the target loader and libraries in the SDK.
hyprcross supplies both the loader prefix and library search path. If execution fails, confirm
that `hyprcross sdk path <target>` exists and reinstall that SDK before changing runner flags.
