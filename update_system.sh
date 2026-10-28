#!/bin/bash
# update_system.sh - быстрое обновление системного раздела

set -e

IMG_NAME="cubeos.img"

if [ ! -f $IMG_NAME ]; then
    echo "ОШИБКА: $IMG_NAME не найден!"
    echo "Сначала создай HDD образ: ./create_hdd.sh"
    exit 1
fi

if [ ! -f kernel.elf ]; then
    echo "ОШИБКА: kernel.elf не найден!"
    echo "Сначала собери ядро: make"
    exit 1
fi

echo "=== Обновление системного раздела ==="

# Private temp mount points so a pre-existing /mnt symlink cannot redirect writes
MNT_BOOT=$(mktemp -d)
MNT_CUBEOS=$(mktemp -d)
LOOP_DEV=""

cleanup() {
    sudo umount "$MNT_BOOT" "$MNT_CUBEOS" 2>/dev/null || true
    [ -n "$LOOP_DEV" ] && sudo losetup -d "$LOOP_DEV" 2>/dev/null || true
    rmdir "$MNT_BOOT" "$MNT_CUBEOS" 2>/dev/null || true
}
trap cleanup EXIT

# Монтируем образ
LOOP_DEV=$(sudo losetup -fP --show "$IMG_NAME")
echo "Loop device: $LOOP_DEV"

sudo mount "${LOOP_DEV}p2" "$MNT_CUBEOS"

# Обновляем ядро
echo "Обновление ядра..."
sudo cp kernel.elf "$MNT_CUBEOS/system/kernel/cubecore_1-0-0-dev.elf"

# Также обновляем ядро в /boot для GRUB
sudo mount "${LOOP_DEV}p1" "$MNT_BOOT"
sudo cp kernel.elf "$MNT_BOOT/kernel.elf"
sudo umount "$MNT_BOOT"

echo "Добавление пользовательских приложений..."
sudo cp target/apps/x86_64-unknown-none/release/csh "$MNT_CUBEOS/bin/csh"

echo "Обновление boot.cfg..."
sudo tee $MNT_CUBEOS/system/config/boot.cfg > /dev/null << 'EOF'
# CubeOS Boot Configuration
mode=normal
vesa=true
cmdline=""
EOF

# Размонтируем
sudo umount "$MNT_CUBEOS"
sudo losetup -d "$LOOP_DEV"
LOOP_DEV=""

echo "=== Системный раздел успешно обновлён ==="
