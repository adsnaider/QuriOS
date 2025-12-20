set dotenv-load
set positional-arguments
set export

arch := env_var("ARCH")
profile := env_var("PROFILE")
debugger := "no"

[private]
target := arch + "-unknown-none"

artifact_dir := ".build"
build_dir := artifact_dir / profile
image_path := build_dir / "qurios.iso"
test_image_path := build_dir / "qurios-test.iso"

[private]
extractor := "jq -r '.filenames | last' | tail -2 | head -1"
[private]
iso_root := build_dir / "iso_root"
qemu_user_args := env_var_or_default("QEMU_ARGS", "")
[private]
qemu_args := if debugger == "yes" { qemu_user_args + " -s -S" } else { qemu_user_args }

default: iso

install-deps:
	#!/bin/bash
	if ! command -v cargo; then
		curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
	fi
	sudo apt update && sudo apt install qemu-system-x86 ovmf mkisofs xorriso


check:
	cargo clippy --workspace --target {{target}}
	cargo clippy --workspace --tests
	cargo +nightly fmt --all --check

fix:
	cargo fix --workspace --target {{target}}

clippy:
	cargo clippy  --workspace --target {{target}}
	cargo clippy  --workspace --tests

fmt:
	cargo +nightly fmt --all

setup:
	rm -rf {{build_dir}}
	mkdir -p {{build_dir}}

init: setup
	#!/usr/bin/env bash
	set -euo pipefail
	export RUSTFLAGS="-Crelocation-model=static -Cforce-frame-pointers=yes"
	BIN=`cargo build -p init --profile {{profile}} --target {{target}} --bin init --message-format=json | {{extractor}}`
	cp -f "$BIN" "{{build_dir}}/init"

initrd: init
	cd {{build_dir}} && tar -H ustar -cf initrd.tar "init"

kernel: setup
	#!/usr/bin/env bash
	export RUSTFLAGS="-Cforce-frame-pointers=yes"
	set -euo pipefail
	BIN=`cargo build --profile {{profile}} --target {{target}} --bin kmain --message-format=json| {{extractor}}`
	cp -f "$BIN" "{{build_dir}}/kernel"

build: kernel initrd

limine:
	{{path_exists("limine/")}} || git clone https://github.com/limine-bootloader/limine.git --branch=v9.x-binary --depth=1
	make -C limine

iso: limine build
	just profile={{profile}} arch={{arch}} artifact_dir={{artifact_dir}} iso_generic {{build_dir}}/kernel limine.conf {{image_path}}

test-iso: limine build
	just profile={{profile}} arch={{arch}} artifact_dir={{artifact_dir}} iso_generic {{build_dir}}/kernel_test limine-test.conf {{test_image_path}}

dbg_dir: setup
	mkdir -p {{artifact_dir}}/debugger/
	ln -sf ../{{profile}}/kernel {{artifact_dir}}/debugger
	ln -sf ../{{profile}}/init {{artifact_dir}}/debugger

emulate: dbg_dir iso
	@./go.sh 33 qemu-system-x86_64 \
		-cdrom {{image_path}} \
		-bios /usr/share/ovmf/OVMF.fd \
		-chardev stdio,id=char0,logfile={{artifact_dir}}/serial.log,signal=off \
		-serial chardev:char0 \
		{{qemu_args}} | tee .build/serial.out
	@awk '/======== BACKTRACE ========/{flag=1; next}/======== END BACKTRACE ========/{flag=0} flag' .build/serial.out | xargs addr2line -C -f -p -i -e {{artifact_dir}}/debugger/kernel

	

ktest: test-iso
	@./go.sh 33 qemu-system-x86_64 \
		-cdrom {{test_image_path}} \
		-bios /usr/share/ovmf/OVMF.fd \
		-chardev stdio,id=char0,logfile={{artifact_dir}}/serial.log,signal=off \
		-serial chardev:char0 \
		-device isa-debug-exit,iobase=0xf4,iosize=0x04 \
		-display none \
		{{qemu_args}}

test:
	cargo test --workspace

clean:
	rm -rf {{artifact_dir}}
	rm -rf limine/
	cargo clean

[private]
iso_generic kernel_path limine_cfg output_path: limine build initrd
	rm -rf {{iso_root}}
	mkdir -p {{iso_root}}/boot
	cp -v "$kernel_path" {{iso_root}}/boot
	cp -v {{build_dir}}/initrd.tar {{iso_root}}/boot
	mkdir -p {{iso_root}}/boot/limine
	cp -v "$limine_cfg" limine/limine-bios.sys limine/limine-bios-cd.bin limine/limine-uefi-cd.bin {{iso_root}}/boot/limine
	mkdir -p {{iso_root}}/EFI/BOOT
	cp -v limine/BOOTX64.EFI {{iso_root}}/EFI/BOOT/
	cp -v limine/BOOTIA32.EFI {{iso_root}}/EFI/BOOT/
	xorriso -as mkisofs -b boot/limine/limine-bios-cd.bin \
		-no-emul-boot -boot-load-size 4 -boot-info-table \
		--efi-boot boot/limine/limine-uefi-cd.bin \
		-efi-boot-part --efi-boot-image --protective-msdos-label \
		{{iso_root}} -o "$output_path"
	@echo "ISO Image Built: $output_path"
