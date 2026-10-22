TARGET = x86_64-unknown-none
KERNEL = kernel.elf
ISO = cubeos.iso
HDD = cubeos.img
CC = gcc
CARGO = cargo +nightly
NASM = nasm
LD = ld
 

# Путь к библиотеке — в корне проекта в target/
LIB_PATH = target/$(TARGET)/release/libcube_core.a

.PHONY: all clean run hdd update-system run-hdd FORCE

all: $(KERNEL)

# Компиляция bootloader
boot.o: src/cube_core/boot.asm
	$(NASM) -f elf64 $< -o $@

ata.o: src/cube_core/ata.c
	gcc -ffreestanding -mno-red-zone -m64 -fno-stack-protector -c src/cube_core/ata.c -o ata.o

# Компиляция ядра (cargo запускается из src/cube_core, но target/ создаётся в корне)
$(LIB_PATH): FORCE
	cd src/cube_core && RUSTFLAGS="-C link-arg=-nostdlib -C relocation-model=static -C code-model=kernel" \
		$(CARGO) build -Z build-std=core,compiler_builtins -Z build-std-features=compiler-builtins-mem \
		--target $(TARGET) --release --target-dir ../../target

FORCE:

# Линковка
$(KERNEL): boot.o ata.o $(LIB_PATH) linker.ld
	$(LD) -T linker.ld -o $@ boot.o ata.o $(LIB_PATH) \
		--gc-sections -z max-page-size=0x1000
	@echo "  Kernel linked: $(KERNEL)"

# Создание ISO
$(ISO): $(KERNEL)
	@mkdir -p iso/boot/grub
	cp $(KERNEL) iso/boot/kernel.elf
	@echo 'set timeout=3' > iso/boot/grub/grub.cfg
	@echo 'set default=0' >> iso/boot/grub/grub.cfg
	@echo 'menuentry "CubeOS v1.0" {' >> iso/boot/grub/grub.cfg
	@echo '    multiboot2 /boot/kernel.elf' >> iso/boot/grub/grub.cfg
	@echo '}' >> iso/boot/grub/grub.cfg
	grub-mkrescue -o $@ iso 2>/dev/null
	@echo "  ISO created: $(ISO)"

run: $(ISO)
	qemu-system-x86_64 -cdrom $(ISO) -m 256M -serial std -no-reboot -no-shutdown

clean:
	rm -rf target
	rm -rf src/cube_core/target
	rm -rf iso $(ISO) $(KERNEL) boot.o
	@echo "  Cleaned"
