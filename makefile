KERNEL ?= /boot/vmlinuz

target/root.img: target/init
	qemu-img create $@ 100M
	mkfs.ext2 $@
	e2mkdir $@:/dev
	e2mkdir $@:/proc
	e2mkdir $@:/root
	e2cp -P 755 init $@:/

target/init: target/debug/rustos
	cp $< $@

target/debug/rustos: export RUSTFLAGS = -C target-feature=+crt-static
target/debug/rustos: $(shell find src)
	cargo build

run: target/root.img
	qemu-system-x86_64 \
		-m 128M \
		-kernel $(KERNEL) \
		-drive file=target/root.img,if=none,format=raw,media=disk,id=r1 \
		-device virtio-blk,drive=r1 \
		-append "init=/init root=/dev/vda console=ttyS0,115200" \
		-serial stdio \
		-display none
