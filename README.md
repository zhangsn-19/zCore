# zircon-rs

[![Actions Status](https://github.com/rcore-os/zircon-rs/workflows/CI/badge.svg)](https://github.com/rcore-os/zircon-rs/actions)
[![Coverage Status](https://coveralls.io/repos/github/rcore-os/zircon-rs/badge.svg?branch=master)](https://coveralls.io/github/rcore-os/zircon-rs?branch=master)

Reimplement [Zircon][zircon] microkernel in safe Rust as a userspace program!

🚧 Working In Progress


## Components

* `zircon-object`: Kernel objects.

  This is the core of the whole project.
  
  It implements all Zircon [kernel objects][kernel-objects].
  
* `zircon-syscall`: Zircon syscall layer.

  It implements Zircon [syscalls][syscalls] using the above objects.

* `zircon-userboot`: Zircon user program loader.

* `linux-syscall`: Linux syscall layer.

* `linux-loader`: Linux user program loader.

* `kernel-hal-unix`: HAL implementation on Unix.

  It's used for unit testing and construct a libOS.

* `kernel-hal-bare`: HAL implementation on bare metal environment.

  It's used to construct a real Zircon "kernel" -- zCore.

[zircon]: https://fuchsia.googlesource.com/fuchsia/+/master/zircon/README.md
[kernel-objects]: https://github.com/PanQL/zircon/blob/master/docs/objects.md
[syscalls]: https://github.com/PanQL/zircon/blob/master/docs/syscalls.md

