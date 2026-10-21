#!/bin/bash
# tools/mkimg.sh - Создание образа HDD с разделами

set -e

IMAGE="$1"
SIZE="${2:-1024}"  # Размер в МБ

if [ -z "$IMAGE" ]; then
    echo "Usage: $0 <image-file> [size-mb]"
    exit 1
fi

echo "Creating HDD image: $IMAGE ($SIZE MB)"

# Создаем пустой образ
dd if=/dev/zero of="$IMAGE" bs=1M count="$SIZE" status=progress

# Создаем разделы
parted -s "$IMAGE" mklabel msdos
parted -s "$IMAGE" mkpart primary fat32 1MiB 101MiB
parted -s "$IMAGE" mkpart primary ext2 101MiB 100%

# Форматируем
LOOP=$(sudo losetup -fP --show "$IMAGE")
sudo mkfs.fat -F32 -n "CUBEOS_BOOT" "${LOOP}p1"
sudo mkfs.ext2 -L "CUBEOS_SYS" "${LOOP}p2"

echo "Image created: $IMAGE"
sudo losetup -d "$LOOP"
