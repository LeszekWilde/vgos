KERNEL := target/x86_64-kernel/debug/vgos
ISO := vgos.iso
LIMINE_VERSION = 12.3.2

.PHONY: all run clean

all: $(ISO)

$(KERNEL):
	cargo build

limine:
	curl -sLO https://github.com/limine-bootloader/limine/releases/download/v$(LIMINE_VERSION)/limine-binary.tar.xz
	mkdir -p limine
	tar -xf limine-binary.tar.xz -C limine --strip-components=1
	rm limine-binary.tar.xz
	$(MAKE) -C limine

$(ISO): $(KERNEL) limine
	rm -rf iso_dir
	mkdir -p iso_dir/boot/limine
	mkdir -p iso_dir/EFI/BOOT

	cp $(KERNEL) iso_dir/kernel
	cp limine.conf iso_dir/

	cp limine/limine-bios.sys limine/limine-bios-cd.bin limine/limine-uefi-cd.bin iso_dir/boot/limine/
	cp limine/BOOTX64.EFI limine/BOOTIA32.EFI iso_dir/EFI/BOOT/

	xorriso -as mkisofs -b boot/limine/limine-bios-cd.bin \
		-no-emul-boot -boot-load-size 4 -boot-info-table \
		--efi-boot boot/limine/limine-uefi-cd.bin \
		-efi-boot-part --efi-boot-image --protective-msdos-label \
		iso_dir -o $(ISO)

	./limine/limine bios-install $(ISO)

run: $(ISO)
	qemu-system-x86_64 -M q35 -m 2G -cdrom $(ISO) -boot d -no-reboot -no-shutdown

clean:
	cargo clean
	rm -rf iso_dir $(ISO) limine
