# VGOS (Very Good Operating System)

VGOS is an experimental, 64-bit bare-metal operating system written entirely in Rust. It is currently in active development, targeting the x86_64 architecture with a focus on modern, secure, and performant microkernel design.

It boots via the [Limine](https://limine-bootloader.org/) bootloader protocol, completely bypassing legacy VGA text mode in favor of a modern, high-resolution linear framebuffer.

## Prerequisites

To build and run VGOS, you must be on a Unix-like environment (Linux or macOS) and have the following tools installed:

### 1. Rust Nightly
The kernel relies on unstable features (`build-std`, `abi_x86_interrupt`).
```bash
rustup override set nightly
rustup component add rust-src
```

### 2. QEMU
Required for hardware virtualization and testing.
- **macOS:** `brew install qemu`
- **Linux:** `sudo apt install qemu-system-x86`

### 3. Xorriso
Required to pack the kernel and bootloader into a bootable ISO.
- **macOS:** `brew install xorriso`
- **Linux:** `sudo apt install xorriso`

### 4. GNU Make
Apple's default `make` is severely outdated. If on macOS, install `gmake`.
- **macOS:** `brew install make`

## Building and Running

The build system is entirely automated via a central `Makefile`. It will automatically download the Limine bootloader binaries, compile the Rust kernel for the custom `x86_64-kernel` target, stitch the ISO together, and launch it in QEMU.
If on macOS, use `gmake run`. On Linux, use `make run`.


To clean the build artifacts and remove the downloaded bootloader binaries:
- **macOS:** `gmake clean`
- **Linux:** `make clean`

## License

VGOS is free and open-source software distributed under the terms of the GNU General Public License v3.0 (GPLv3). See the `LICENSE` file for details.
