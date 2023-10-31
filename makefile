KERNEL ?= /boot/vmlinuz
TARGET ?= x86_64-unknown-linux-gnu

target/root.img: target/$(TARGET)/init
	qemu-img create $@ 2000M
	mkfs.ext2 $@
	e2mkdir $@:/dev
	e2mkdir $@:/sys
	e2mkdir $@:/tmp
	e2mkdir $@:/proc

	e2mkdir $@:/root
	e2mkdir $@:/etc
	e2mkdir $@:/etc/ssl/certs
	e2mkdir $@:/bin
	e2mkdir $@:/sbin
	e2mkdir $@:/usr/bin
	e2mkdir $@:/usr/sbin
	e2mkdir $@:/var/log

	e2cp -P 755 target/$(TARGET)/init $@:/
	e2cp -P 755 target/busybox $@:/
	e2cp -P 755 target/k3s $@:/
	e2cp -P 755 target/ca-certificates.crt $@:/etc/ssl/certs/

target/$(TARGET)/init: target/$(TARGET)/debug/linμos
	cp $< $@

target/$(TARGET)/debug/linμos: export RUSTFLAGS = -C target-feature=+crt-static
target/$(TARGET)/debug/linμos: $(shell find src)
	cargo build --target $(TARGET)

run: target/root.img
	qemu-system-x86_64 \
		-m 128M \
		-kernel $(KERNEL) \
		-drive file=target/root.img,if=none,format=raw,media=disk,id=r1 \
		-device virtio-blk,drive=r1 \
		-append "init=/init root=/dev/vda console=ttyS0,115200 dwc_otg.lpm_enable=0 cgroup_enable=cpuset cgroup_enable=memory cgroup_memory=1" \
		-serial stdio \
		-display none
