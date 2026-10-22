#!/bin/bash
# create_hdd.sh - создание HDD образа CubeOS

set -e

IMG_NAME="cubeos.img"
IMG_SIZE="512M"
BOOT_SIZE="200M"

echo "=== Создание HDD образа CubeOS ==="
echo "Размер: $IMG_SIZE"
echo "Загрузочный раздел: $BOOT_SIZE (FAT32, /boot)"
echo "Системный раздел: остальное (FAT32, /)"

# Удаляем старый образ
rm -f $IMG_NAME

# Создаём пустой образ
echo "[1/6] Создание образа..."
dd if=/dev/zero of=$IMG_NAME bs=1M count=512 status=progress

# Создаём таблицу разделов MBR
echo "[2/6] Создание разделов..."
/usr/sbin/parted -s $IMG_NAME mklabel msdos
/usr/sbin/parted -s $IMG_NAME mkpart primary fat32 1MiB 201MiB
/usr/sbin/parted -s $IMG_NAME mkpart primary fat32 201MiB 100%
/usr/sbin/parted -s $IMG_NAME set 1 boot on

# Монтируем образ через loop device
echo "[3/6] Монтирование образа..."
LOOP_DEV=$(sudo losetup -fP --show $IMG_NAME)
echo "Loop device: $LOOP_DEV"

# Форматируем разделы
echo "[4/6] Форматирование разделов..."
sudo mkfs.fat -F 32 ${LOOP_DEV}p1 -n BOOT
sudo mkfs.fat -F 32 ${LOOP_DEV}p2 -n CUBEOS

# Монтируем разделы
sudo mkdir -p /mnt/boot
sudo mkdir -p /mnt/cubeos
sudo mount ${LOOP_DEV}p1 /mnt/boot
sudo mount ${LOOP_DEV}p2 /mnt/cubeos

# Устанавливаем GRUB на первый раздел
echo "[5/6] Установка GRUB..."
sudo grub-install --target=i386-pc --boot-directory=/mnt/boot --recheck $LOOP_DEV

# Создаём директорию для ядра в /boot (для GRUB)
sudo mkdir -p /mnt/boot/grub

# Копируем ядро в /boot (для GRUB)
if [ -f kernel.elf ]; then
    sudo cp kernel.elf /mnt/boot/
    echo "Ядро скопировано в /boot/kernel.elf"
else
    echo "ОШИБКА: kernel.elf не найден! Сначала выполни 'make'"
    sudo umount /mnt/boot /mnt/cubeos
    sudo losetup -d $LOOP_DEV
    exit 1
fi

# Создаём grub.cfg
sudo tee /mnt/boot/grub/grub.cfg > /dev/null << 'EOF'
set timeout=3
set default=0

menuentry "CubeOS v1.0" {
    insmod part_msdos
    insmod fat
    set root=(hd0,msdos2)
    multiboot2 /system/kernel/cubecore_1-0-0-dev.elf
}

menuentry "CubeOS v1.0 (Debug)" {
    insmod part_msdos
    insmod fat
    set root=(hd0,msdos2)
    multiboot2 /system/kernel/cubecore_1-0-0-dev.elf debug
}
EOF

# Создаём структуру системного раздела (/)
echo "[6/6] Создание системной структуры..."
sudo mkdir -p /mnt/cubeos/system/kernel
sudo mkdir -p /mnt/cubeos/system/config
sudo mkdir -p /mnt/cubeos/home/user
sudo mkdir -p /mnt/cubeos/tmp
sudo mkdir -p /mnt/cubeos/bin
sudo mkdir -p /mnt/cubeos/etc

# Копируем ядро в системный раздел
sudo cp kernel.elf /mnt/cubeos/system/kernel/cubecore_1-0-0-dev.elf
echo "Ядро скопировано в /system/kernel/cubecore_1-0-0-dev.elf"

# Создаём базовые конфигурационные файлы
sudo tee /mnt/cubeos/system/config/boot.cfg > /dev/null << 'EOF'
# CubeOS Boot Configuration
mode=normal
vesa=true
cmdline="/bin/csh"
EOF

sudo tee /mnt/cubeos/system/config/debug.cfg > /dev/null << 'EOF'
# CubeOS Debug Configuration
mode=debug
vesa=true
verbose=true
EOF

# Создаём /etc/fstab (таблица файловых систем)
sudo tee /mnt/cubeos/etc/fstab > /dev/null << 'EOF'
# CubeOS fstab
# <device>    <mountpoint>    <type>    <options>
/dev/hda1     /boot           fat32     ro
/dev/hda2     /               fat32     rw
EOF

# Создаём приветственный файл
sudo tee /mnt/cubeos/etc/motd > /dev/null << 'EOF'
Welcome to CubeOS v1.0!
Type 'help' for available commands.
EOF

# Размонтируем
echo "Завершение..."
sudo umount /mnt/boot
sudo umount /mnt/cubeos
sudo losetup -d $LOOP_DEV

echo ""
echo "=== HDD образ успешно создан: $IMG_NAME ==="
echo "Структура:"
echo "  /boot (200MB) - GRUB и ядро для загрузки"
echo "  / (остальное) - системный раздел"
echo "    /system/kernel/cubecore_1-0-0-dev.elf"
echo "    /system/config/"
echo "    /home/user/"
echo "    /tmp/"
echo "    /bin/"
echo "    /etc/"
