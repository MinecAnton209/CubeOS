#!/bin/bash
# build.sh - CubeOS v1.0 HDD сборка через GNU ld

set -e

echo "╔══════════════════════════════════════════════════════════╗"
echo "║     CubeOS v1.0 - Сборка и создание HDD образа        ║"
echo "╚══════════════════════════════════════════════════════════╝"

IMG="cubeos.img"
SIZE=100

echo ""
echo "[1/7] Очистка..."
rm -f boot.o ata.o libkernel.a kernel.elf "$IMG"

echo ""
echo "[2/7] Сборка загрузчика (boot.asm)..."
nasm -f elf64 src/cube_core/boot.asm -o boot.o

echo ""
echo "[3/7] Сборка ATA драйвера (ata.c)..."
gcc -ffreestanding -mno-red-zone -m64 -c src/cube_core/ata.c -o ata.o

echo ""
echo "[4/7] Сборка Rust ядра (CubeCORE)..."
cd src/cube_core
RUSTFLAGS="-C link-arg=-nostdlib -C relocation-model=static -C code-model=kernel" \
cargo +nightly build -Z build-std=core,compiler_builtins \
    -Z build-std-features=compiler-builtins-mem \
    --target x86_64-unknown-none \
    --release 2>&1 | grep -v "warning:" || true
cd ../..
cp src/cube_core/target/x86_64-unknown-none/release/libcube_core.a libkernel.a

echo ""
echo "[5/7] Линковка ядра через GNU ld..."
ld -m elf_x86_64 -T src/cube_core/linker.ld -o kernel.elf \
    boot.o ata.o libkernel.a \
    -z max-page-size=0x1000 \
    --no-gc-sections

echo ""
echo "[6/7] Создание HDD образа ($SIZE МБ)..."
dd if=/dev/zero of="$IMG" bs=1M count=$SIZE status=progress 2>&1

# Создаем MBR и раздел
sudo parted -s "$IMG" mklabel msdos
sudo parted -s "$IMG" mkpart primary fat32 2048s 100%

# Подключаем через loop
LOOP=$(sudo losetup -fP --show "$IMG")
echo "Loop устройство: $LOOP"

# Форматируем как FAT32
sudo mkfs.fat -F32 -n "CUBEOS" "${LOOP}p1"

# Монтируем
sudo mkdir -p /tmp/cubeos
sudo mount "${LOOP}p1" /tmp/cubeos

# Устанавливаем GRUB
echo "Установка GRUB..."
sudo grub-install --target=i386-pc --boot-directory=/tmp/cubeos/boot "$LOOP"

# Создаем структуру директорий
echo "Создание структуры директорий..."
sudo mkdir -p /tmp/cubeos/.System/kernel
sudo mkdir -p /tmp/cubeos/.System/drivers
sudo mkdir -p /tmp/cubeos/.System/config
sudo mkdir -p /tmp/cubeos/.System/Logs
sudo mkdir -p /tmp/cubeos/.PPPD
sudo mkdir -p /tmp/cubeos/.SysApps
sudo mkdir -p /tmp/cubeos/Apps
sudo mkdir -p /tmp/cubeos/Users/Admin
sudo mkdir -p /tmp/cubeos/Users/Guest
sudo mkdir -p /tmp/cubeos/.System/drivers

# Копируем ядро
echo "Копирование ядра..."
sudo cp kernel.elf /tmp/cubeos/.System/kernel/kernel.bin

echo "Копирование драйверов..."

# Создаем grub.cfg
echo "Создание grub.cfg..."
sudo bash -c 'cat > /tmp/cubeos/boot/grub/grub.cfg << EOF
set timeout=3
set default=0

menuentry "CubeOS v1.0" {
    multiboot2 /.System/kernel/kernel.bin
}

menuentry "CubeOS v1.0 (Debug)" {
    multiboot2 /.System/kernel/kernel.bin
    module2 /.System/config/debug.cfg
}
EOF'

# Создаем конфиги
sudo bash -c 'cat > /tmp/cubeos/.System/config/system.cfg << EOF
version=1.0.0
kernel=/.System/kernel/kernel.bin
root=/dev/sda1
EOF'

sudo bash -c 'cat > /tmp/cubeos/.System/config/debug.cfg << EOF
mode=debug
verbose=true
EOF'

# Создаем файл README для пользователя
sudo bash -c 'cat > /tmp/cubeos/README.txt << EOF
CubeOS v1.0

Структура:
- .System/     - системные файлы (скрытый)
- .PPPD/       - защищенные данные (скрытый)
- .SysApps/    - системные приложения (скрытый)
- Apps/        - пользовательские приложения
- Users/       - пользовательские данные

Для разработчиков:
- Исходный код: https://github.com/cubeweb-lab/cubeos
- Лицензия: GPLv3
EOF'

echo ""
echo "[7/7] Завершение..."
sudo umount /tmp/cubeos
sudo losetup -d "$LOOP"
rm -rf /tmp/cubeos 2>/dev/null || true

echo ""
echo "╔══════════════════════════════════════════════════════════╗"
echo "║     ✅ Сборка завершена успешно!                       ║"
echo "║     Образ: $IMG ($(du -h $IMG | cut -f1))            ║"
echo "║     Ядро: kernel.elf ($(du -h kernel.elf | cut -f1)) ║"
echo "╚══════════════════════════════════════════════════════════╝"
echo ""

echo "Запуск QEMU..."
qemu-system-x86_64 \
    -drive file="$IMG",format=raw \
    -m 1024M \
    -vga vmware \
    -no-reboot \
    -no-shutdown
