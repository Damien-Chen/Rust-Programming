# Rust UEFI BMP Image Viewer

This project is a simple UEFI application written in Rust that displays a BMP image (`\\image.bmp`) on the screen. It serves as a basic example of UEFI development in Rust, demonstrating graphics output and file system access.

## Features

*   Displays a 24-bit or 32-bit uncompressed BMP image.
*   Centers the image on the screen.
*   Basic logging support.
*   Panic handler for easier debugging.

## Requirements

*   Rust toolchain (with `rust-src` component for `build-std`)
*   `cargo-binutils`
*   QEMU (for running the UEFI application)
*   An OVMF (Open Virtual Machine Firmware) file.

You can install the required Rust components with:

```bash
rustup component add rust-src
cargo install cargo-binutils
```

## Building and Running

1.  **Build the project:**

    ```bash
    cargo build --release
    ```

2.  **Create a disk image:** You will need to create a FAT-formatted disk image and place the compiled EFI executable and a BMP image on it.

    *   Create a directory for the disk image contents:
        ```bash
        mkdir -p esp/efi/boot
        ```
    *   Copy the compiled EFI executable to the disk image directory:
        ```bash
        cp target/x86_64-unknown-uefi/release/main.efi esp/efi/boot/bootx64.efi
        ```
    *   Place your BMP image at the root of the disk image directory:
        ```bash
        cp your_image.bmp esp/image.bmp
        ```
    *   Create the disk image:
        ```bash
        dd if=/dev/zero of=fat.img bs=1k count=1440
        mformat -i fat.img -f 1440 ::
        mcopy -i fat.img -s esp/* ::
        ```

3.  **Run with QEMU:**

    ```bash
    qemu-system-x86_64 -drive if=pflash,format=raw,file=/path/to/your/OVMF.fd -drive format=raw,file=fat.img
    ```

## Dependencies

*   [log](https://crates.io/crates/log): A logging facade for Rust.
*   [uefi](https://crates.io/crates/uefi): A crate for writing UEFI applications in Rust.