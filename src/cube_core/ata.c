// src/ata.c – с таймаутами и защитой от зависания
#include <stdint.h>

#define ATA_DATA    0x1F0
#define ATA_ERROR   0x1F1
#define ATA_COUNT   0x1F2
#define ATA_LBA0    0x1F3
#define ATA_LBA1    0x1F4
#define ATA_LBA2    0x1F5
#define ATA_DRIVE   0x1F6
#define ATA_COMMAND 0x1F7
#define ATA_STATUS  0x1F7

#define ATA_CMD_IDENTIFY 0xEC
#define ATA_CMD_READ     0x20
#define ATA_CMD_WRITE    0x30

#define STATUS_BSY  0x80
#define STATUS_DRDY 0x40
#define STATUS_DRQ  0x08
#define STATUS_ERR  0x01

static unsigned char inb(unsigned short port) {
    unsigned char value;
    __asm__ volatile ("inb %1, %0" : "=a"(value) : "Nd"(port));
    return value;
}

static void outb(unsigned short port, unsigned char value) {
    __asm__ volatile ("outb %0, %1" : : "a"(value), "Nd"(port));
}

static unsigned short inw(unsigned short port) {
    unsigned short value;
    __asm__ volatile ("inw %1, %0" : "=a"(value) : "Nd"(port));
    return value;
}

// Отладочный вывод в порт 0xE9
static void debug_char(char c) {
    __asm__ volatile ("outb %0, %1" : : "a"(c), "Nd"(0xE9));
}

// Ожидание с таймаутом (не более ~1 секунды)
static int ata_wait_ready(void) {
    int timeout = 1000000; // ~1M итераций
    while (inb(ATA_STATUS) & STATUS_BSY) {
        timeout--;
        if (timeout <= 0) {
            debug_char('T'); // Timeout
            return 0; // Таймаут
        }
    }
    return 1; // Готов
}

static int ata_wait_drq(void) {
    int timeout = 1000000;
    while (!(inb(ATA_STATUS) & STATUS_DRQ)) {
        timeout--;
        if (timeout <= 0) {
            debug_char('U'); // DRQ Timeout
            return 0;
        }
    }
    return 1;
}

int ata_init_c(void) {
    unsigned char status;

    debug_char('X'); // Начало ata_init_c

    // Сбрасываем контроллер
    outb(ATA_DRIVE, 0xA0);
    for (int i = 0; i < 100; i++) inb(ATA_STATUS);

    // Проверка на floating bus
    status = inb(ATA_STATUS);
    if (status == 0xFF || status == 0x00) {
        debug_char('F'); // Floating bus
        return 0;
    }

    debug_char('Y'); // Статус ок

    // Отправляем IDENTIFY
    outb(ATA_DRIVE, 0xA0);
    outb(ATA_COUNT, 0);
    outb(ATA_LBA0, 0);
    outb(ATA_LBA1, 0);
    outb(ATA_LBA2, 0);
    outb(ATA_COMMAND, ATA_CMD_IDENTIFY);

    // Проверяем статус сразу после команды
    status = inb(ATA_STATUS);
    if (status == 0) {
        debug_char('Z'); // Статус 0
        return 0;
    }

    // Ждём готовности с таймаутом
    if (!ata_wait_ready()) {
        debug_char('W'); // Таймаут ожидания
        return 0;
    }

    debug_char('R'); // Готов к чтению IDENTIFY

    // Читаем 256 слов
    unsigned short identify_data[256];
    for (int i = 0; i < 256; i++) {
        identify_data[i] = inw(ATA_DATA);
    }

    // Проверяем, что это ATA (не ATAPI)
    if (identify_data[0] & 0x8000) {
        debug_char('A'); // ATAPI
        return 0;
    }

    debug_char('D'); // Диск обнаружен
    return 1;
}

int ata_read_sector_c(unsigned int lba, unsigned char *buf) {
    if (!ata_wait_ready()) return 0;

    outb(ATA_DRIVE, 0xE0 | ((lba >> 24) & 0x0F));
    outb(ATA_COUNT, 1);
    outb(ATA_LBA0, lba & 0xFF);
    outb(ATA_LBA1, (lba >> 8) & 0xFF);
    outb(ATA_LBA2, (lba >> 16) & 0xFF);
    outb(ATA_COMMAND, ATA_CMD_READ);

    if (!ata_wait_drq()) return 0;

    for (int i = 0; i < 256; i++) {
        unsigned short word = inw(ATA_DATA);
        buf[i * 2] = word & 0xFF;
        buf[i * 2 + 1] = (word >> 8) & 0xFF;
    }

    if (inb(ATA_STATUS) & STATUS_ERR) return 0;
    return 1;
}

int ata_write_sector_c(unsigned int lba, const unsigned char *buf) {
    if (!ata_wait_ready()) return 0;

    outb(ATA_DRIVE, 0xE0 | ((lba >> 24) & 0x0F));
    outb(ATA_COUNT, 1);
    outb(ATA_LBA0, lba & 0xFF);
    outb(ATA_LBA1, (lba >> 8) & 0xFF);
    outb(ATA_LBA2, (lba >> 16) & 0xFF);
    outb(ATA_COMMAND, ATA_CMD_WRITE);

    if (!ata_wait_drq()) return 0;

    for (int i = 0; i < 256; i++) {
        unsigned short word = (buf[i * 2 + 1] << 8) | buf[i * 2];
        __asm__ volatile ("outw %0, %1" : : "a"(word), "d"((uint16_t)ATA_DATA));
    }

    if (inb(ATA_STATUS) & STATUS_ERR) return 0;
    return 1;
}
