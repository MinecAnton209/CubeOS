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

# Монтируем образ
LOOP_DEV=$(sudo losetup -fP --show $IMG_NAME)
echo "Loop device: $LOOP_DEV"

sudo mkdir -p /mnt/cubeos
sudo mount ${LOOP_DEV}p2 /mnt/cubeos

# Обновляем ядро
echo "Обновление ядра..."
sudo cp kernel.elf /mnt/cubeos/system/kernel/cubecore_1-0-0-dev.elf

# Также обновляем ядро в /boot для GRUB
sudo mkdir -p /mnt/boot
sudo mount ${LOOP_DEV}p1 /mnt/boot
sudo cp kernel.elf /mnt/boot/kernel.elf
sudo umount /mnt/boot

# Размонтируем
sudo umount /mnt/cubeos
sudo losetup -d $LOOP_DEV

echo "=== Системный раздел успешно обновлён ==="
